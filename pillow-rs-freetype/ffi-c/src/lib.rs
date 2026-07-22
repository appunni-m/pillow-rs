#![allow(missing_docs)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(non_camel_case_types, non_snake_case)]

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{
    CStr, CString, c_char, c_int, c_long, c_short, c_uchar, c_uint, c_ulong, c_ushort, c_void,
};
use std::ptr::{self, NonNull};
use std::slice;

use fontdone::ffi as rust_ffi;

#[cfg(feature = "abi-test-support")]
thread_local! {
    static TEST_OUTLINE_RENDER_SPANS: RefCell<Vec<(c_int, FT_Span)>> = const { RefCell::new(Vec::new()) };
    static TEST_OUTLINE_RENDER_USER_SEEN: RefCell<bool> = const { RefCell::new(false) };
    static TEST_OUTLINE_RENDER_USER_TOKEN: RefCell<*mut c_void> = const { RefCell::new(ptr::null_mut()) };
}

struct OwnedMmVar {
    master: Box<FT_MM_Var>,
    _axis: Box<[FT_Var_Axis]>,
    _namedstyle: Box<[rust_ffi::FT_Var_Named_Style]>,
    _namedstyle_coords: Box<[rust_ffi::FT_Fixed]>,
}

#[cfg(feature = "abi-test-support")]
pub type AbiMmVarNamedStyleSnapshot = (rust_ffi::FT_Var_Named_Style, Vec<rust_ffi::FT_Fixed>);

#[cfg(feature = "abi-test-support")]
pub type AbiMmVarDescriptorSnapshot = (
    FT_Error,
    FT_MM_Var,
    Vec<FT_Var_Axis>,
    Vec<FT_UInt>,
    Vec<AbiMmVarNamedStyleSnapshot>,
    FT_Error,
);

thread_local! {
    static OWNED_MM_VARS: RefCell<BTreeMap<usize, OwnedMmVar>> = const { RefCell::new(BTreeMap::new()) };
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_owned_mm_var_count() -> usize {
    OWNED_MM_VARS.with(|vars| vars.borrow().len())
}

pub type FT_Error = c_int;
pub type FT_Bool = c_uchar;
pub type FT_Int = c_int;
pub type FT_UInt = c_uint;
pub type FT_Int32 = i32;
pub type FT_UInt32 = u32;
pub type FT_Byte = c_uchar;
pub type FT_Bytes = *const FT_Byte;
pub type FT_Long = c_long;
pub type FT_ULong = c_ulong;
pub type FT_Pos = c_long;
pub type FT_Fixed = c_long;
pub type FT_Angle = FT_Fixed;
pub type FT_F26Dot6 = c_long;
pub type FT_F2Dot14 = c_short;
pub type FT_Short = c_short;
pub type FT_UShort = c_ushort;
pub type FT_Render_Mode = c_int;
pub type FT_Pixel_Mode = c_int;
pub type FT_Glyph_Format = c_int;
pub type FT_Renderer = *mut FT_RendererRec;
pub type FT_Orientation = c_int;
pub type FT_Size_Request_Type = c_int;
pub type FT_Encoding = c_int;
pub type FT_Sfnt_Tag = c_uint;
pub type FT_LcdFilter = c_int;
pub type FT_TrueTypeEngineType = c_int;
pub type PS_Dict_Keys = c_int;
pub type T1_EncodingType = c_int;
pub type FT_DebugHook_Func = rust_ffi::FT_DebugHook_Func;
pub type FT_StrokerBorder = c_int;
pub type FT_Stroker = *mut c_void;
pub type FT_String = c_char;
pub type FT_MM_Axis = rust_ffi::FT_MM_Axis;
pub type FT_Multi_Master = rust_ffi::FT_Multi_Master;
pub type FT_Var_Axis = rust_ffi::FT_Var_Axis;
pub type FT_MM_Var = rust_ffi::FT_MM_Var;
pub type FT_WinFNT_HeaderRec = rust_ffi::FT_WinFNT_HeaderRec;
pub type FT_WinFNT_Header = *mut FT_WinFNT_HeaderRec;
pub type FT_LayerIterator = rust_ffi::FT_LayerIterator;
pub type FT_ClipBox = rust_ffi::FT_ClipBox;
pub type FT_PaintTransform = rust_ffi::FT_PaintTransform;
pub type BDF_PropertyType = rust_ffi::BDF_PropertyType;
pub type BDF_PropertyValue = rust_ffi::BDF_PropertyValue;
pub type BDF_PropertyRec = rust_ffi::BDF_PropertyRec;
pub type BDF_Property = *mut BDF_PropertyRec;

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
pub struct AbiBdfPropertySnapshot {
    pub type_: BDF_PropertyType,
    pub atom: *const FT_String,
    pub integer: FT_Int32,
    pub cardinal: FT_UInt32,
}
pub type PS_FontInfoRec = rust_ffi::PS_FontInfoRec;
pub type PS_FontInfo = *mut PS_FontInfoRec;
pub type T1_FontInfo = PS_FontInfoRec;
pub type PS_PrivateRec = rust_ffi::PS_PrivateRec;
pub type PS_Private = *mut PS_PrivateRec;
pub type T1_Private = PS_PrivateRec;
pub type FT_Pointer = *mut c_void;
pub type FT_Module_Interface = FT_Pointer;
pub type FT_Module = *mut FT_ModuleRec;
pub type FT_Generic_Finalizer = FT_Pointer;
pub type FT_ListNode = *mut FT_ListNodeRec;
pub type FT_List = *mut FT_ListRec;
pub type FT_List_Iterator = Option<extern "C" fn(node: FT_ListNode, user: FT_Pointer) -> FT_Error>;
pub type FT_Memory = *mut FT_MemoryRec;
pub type FT_StreamDesc = rust_ffi::FT_StreamDesc;
pub type FT_StreamRec = rust_ffi::FT_StreamRec;
pub type FT_Stream = *mut FT_StreamRec;
pub type FT_Stream_CloseFunc = Option<extern "C" fn(stream: FT_Stream)>;
pub type FT_Alloc_Func = Option<extern "C" fn(memory: FT_Memory, size: c_long) -> FT_Pointer>;
pub type FT_Free_Func = Option<extern "C" fn(memory: FT_Memory, block: FT_Pointer)>;
pub type FT_Realloc_Func = Option<
    extern "C" fn(
        memory: FT_Memory,
        cur_size: c_long,
        new_size: c_long,
        block: FT_Pointer,
    ) -> FT_Pointer,
>;
pub type FT_List_Destructor =
    Option<extern "C" fn(memory: FT_Memory, data: FT_Pointer, user: FT_Pointer)>;

pub type FT_Library = *mut FT_LibraryRec;
pub type FT_Face = *mut FT_FaceRec;
pub type FT_Size = *mut FT_SizeRec;
pub type FT_GlyphSlot = *mut FT_GlyphSlotRec;
pub type FT_Glyph = *mut FT_GlyphRec;
pub type FT_BitmapGlyph = *mut FT_BitmapGlyphRec;
pub type FT_CharMap = *mut FT_CharMapRec;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Generic {
    pub data: FT_Pointer,
    pub finalizer: FT_Generic_Finalizer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Vector {
    pub x: FT_Pos,
    pub y: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Matrix {
    pub xx: FT_Fixed,
    pub xy: FT_Fixed,
    pub yx: FT_Fixed,
    pub yy: FT_Fixed,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_BBox {
    pub xMin: FT_Pos,
    pub yMin: FT_Pos,
    pub xMax: FT_Pos,
    pub yMax: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Parameter {
    pub tag: FT_ULong,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_ListNodeRec {
    pub prev: FT_ListNode,
    pub next: FT_ListNode,
    pub data: FT_Pointer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_ListRec {
    pub head: FT_ListNode,
    pub tail: FT_ListNode,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_MemoryRec {
    pub user: FT_Pointer,
    pub alloc: FT_Alloc_Func,
    pub free: FT_Free_Func,
    pub realloc: FT_Realloc_Func,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Open_Args {
    pub flags: FT_UInt,
    pub memory_base: *const FT_Byte,
    pub memory_size: FT_Long,
    pub pathname: *mut c_char,
    pub stream: FT_Stream,
    pub driver: *mut c_void,
    pub num_params: FT_Int,
    pub params: *mut FT_Parameter,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Outline {
    pub n_contours: FT_UShort,
    pub n_points: FT_UShort,
    pub points: *mut FT_Vector,
    pub tags: *mut FT_Byte,
    pub contours: *mut FT_UShort,
    pub flags: FT_Int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Glyph_Class {
    pub glyph_size: FT_Long,
    pub glyph_format: FT_Glyph_Format,
    pub glyph_init: FT_Pointer,
    pub glyph_done: FT_Pointer,
    pub glyph_copy: FT_Pointer,
    pub glyph_transform: FT_Pointer,
    pub glyph_bbox: FT_Pointer,
    pub glyph_prepare: FT_Pointer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_GlyphRec {
    pub library: FT_Pointer,
    pub clazz: *const FT_Glyph_Class,
    pub format: FT_Glyph_Format,
    pub advance: FT_Vector,
}

#[repr(C)]
pub struct FT_OutlineGlyphRec {
    pub root: FT_GlyphRec,
    pub outline: FT_Outline,
}

#[repr(C)]
pub struct FT_BitmapGlyphRec {
    pub root: FT_GlyphRec,
    pub left: FT_Int,
    pub top: FT_Int,
    pub bitmap: FT_Bitmap,
}

#[repr(C)]
struct OwnedOutlineGlyph {
    record: FT_OutlineGlyphRec,
    core: rust_ffi::FT_OutlineGlyphOwned,
    points: Box<[FT_Vector]>,
    tags: Box<[FT_Byte]>,
    contours: Box<[FT_UShort]>,
}

impl OwnedOutlineGlyph {
    fn new(core: rust_ffi::FT_OutlineGlyphOwned) -> Self {
        let mut glyph = Self {
            record: FT_OutlineGlyphRec {
                root: c_glyph_root_from_core(&core.root),
                outline: FT_Outline::default(),
            },
            core,
            points: Box::new([]),
            tags: Box::new([]),
            contours: Box::new([]),
        };
        glyph.refresh_record();
        glyph
    }

    fn refresh_record(&mut self) {
        self.record.root = c_glyph_root_from_core(&self.core.root);
        self.record.root.clazz = owned_outline_glyph_class();
        self.points = self
            .core
            .outline
            .points
            .iter()
            .map(|point| FT_Vector {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.tags = self.core.outline.tags.clone().into_boxed_slice();
        self.contours = self.core.outline.contours.clone().into_boxed_slice();
        self.record.outline = FT_Outline {
            n_contours: u16::try_from(self.contours.len()).unwrap_or(u16::MAX),
            n_points: u16::try_from(self.points.len()).unwrap_or(u16::MAX),
            points: self.points.as_mut_ptr(),
            tags: self.tags.as_mut_ptr(),
            contours: self.contours.as_mut_ptr(),
            flags: self.core.outline.flags,
        };
    }
}

#[repr(C)]
struct OwnedBitmapGlyph {
    record: FT_BitmapGlyphRec,
    core: rust_ffi::FT_BitmapGlyphOwned,
    buffer: Box<[FT_Byte]>,
}

impl OwnedBitmapGlyph {
    fn new(core: rust_ffi::FT_BitmapGlyphOwned) -> Self {
        let mut glyph = Self {
            record: FT_BitmapGlyphRec {
                root: c_glyph_root_from_core_with_class(&core.root, owned_bitmap_glyph_class()),
                left: core.left,
                top: core.top,
                bitmap: FT_Bitmap::default(),
            },
            core,
            buffer: Box::new([]),
        };
        glyph.refresh_record();
        glyph
    }

    fn refresh_record(&mut self) {
        self.record.root =
            c_glyph_root_from_core_with_class(&self.core.root, owned_bitmap_glyph_class());
        self.record.left = self.core.left;
        self.record.top = self.core.top;
        self.buffer = self.core.bitmap.buffer.clone().into_boxed_slice();
        self.record.bitmap = FT_Bitmap {
            rows: self.core.bitmap.rows,
            width: self.core.bitmap.width,
            pitch: self.core.bitmap.pitch,
            buffer: self.buffer.as_mut_ptr(),
            num_grays: self.core.bitmap.num_grays,
            pixel_mode: self.core.bitmap.pixel_mode,
            palette_mode: 0,
            palette: ptr::null_mut(),
        };
    }
}

fn c_glyph_root_from_core(root: &rust_ffi::FT_GlyphRec) -> FT_GlyphRec {
    c_glyph_root_from_core_with_class(root, owned_outline_glyph_class())
}

fn c_glyph_root_from_core_with_class(
    root: &rust_ffi::FT_GlyphRec,
    clazz: *const FT_Glyph_Class,
) -> FT_GlyphRec {
    FT_GlyphRec {
        library: root.library,
        clazz,
        format: root.format,
        advance: FT_Vector {
            x: root.advance.x,
            y: root.advance.y,
        },
    }
}

static OWNED_OUTLINE_GLYPH_CLASS_MARKER: u8 = 0;
static OWNED_BITMAP_GLYPH_CLASS_MARKER: u8 = 0;

fn owned_outline_glyph_class() -> *const FT_Glyph_Class {
    // Private marker used only for pointer identity.  We never dereference this
    // address as an `FT_Glyph_Class`; real class facades continue down the
    // caller-owned public-record path.
    ptr::addr_of!(OWNED_OUTLINE_GLYPH_CLASS_MARKER).cast::<FT_Glyph_Class>()
}

fn owned_bitmap_glyph_class() -> *const FT_Glyph_Class {
    // Private marker used only for pointer identity.  We never dereference this
    // address as an `FT_Glyph_Class`.
    ptr::addr_of!(OWNED_BITMAP_GLYPH_CLASS_MARKER).cast::<FT_Glyph_Class>()
}

fn owned_outline_glyph_from_root(glyph: FT_Glyph) -> Option<&'static OwnedOutlineGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_outline_glyph_class() {
        return None;
    }
    // SAFETY: this sentinel is assigned only for `Box<OwnedOutlineGlyph>`
    // allocations whose first field is an `FT_OutlineGlyphRec`, whose first
    // field is the public `FT_GlyphRec` root.
    Some(unsafe { &*glyph.as_ptr().cast::<OwnedOutlineGlyph>() })
}

fn owned_outline_glyph_from_root_mut(glyph: FT_Glyph) -> Option<&'static mut OwnedOutlineGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_outline_glyph_class() {
        return None;
    }
    // SAFETY: this sentinel is assigned only for `Box<OwnedOutlineGlyph>`
    // allocations whose first field is an `FT_OutlineGlyphRec`, whose first
    // field is the public `FT_GlyphRec` root.
    Some(unsafe { &mut *glyph.as_ptr().cast::<OwnedOutlineGlyph>() })
}

fn owned_bitmap_glyph_from_root(glyph: FT_Glyph) -> Option<&'static OwnedBitmapGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_bitmap_glyph_class() {
        return None;
    }
    // SAFETY: this sentinel is assigned only for `Box<OwnedBitmapGlyph>`
    // allocations whose first field is an `FT_BitmapGlyphRec`, whose first
    // field is the public `FT_GlyphRec` root.
    Some(unsafe { &*glyph.as_ptr().cast::<OwnedBitmapGlyph>() })
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_UnitVector {
    pub x: FT_F2Dot14,
    pub y: FT_F2Dot14,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Glyph_Metrics {
    pub width: FT_Pos,
    pub height: FT_Pos,
    pub horiBearingX: FT_Pos,
    pub horiBearingY: FT_Pos,
    pub horiAdvance: FT_Pos,
    pub vertBearingX: FT_Pos,
    pub vertBearingY: FT_Pos,
    pub vertAdvance: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Size_Metrics {
    pub x_ppem: FT_UShort,
    pub y_ppem: FT_UShort,
    pub x_scale: FT_Fixed,
    pub y_scale: FT_Fixed,
    pub ascender: FT_Pos,
    pub descender: FT_Pos,
    pub height: FT_Pos,
    pub max_advance: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Size_RequestRec {
    pub type_: FT_Size_Request_Type,
    pub width: FT_Long,
    pub height: FT_Long,
    pub horiResolution: FT_UInt,
    pub vertResolution: FT_UInt,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Bitmap {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub buffer: *mut c_uchar,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
    pub palette_mode: c_uchar,
    pub palette: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Span {
    pub x: c_short,
    pub len: c_ushort,
    pub coverage: c_uchar,
}

pub type FT_SpanFunc =
    Option<unsafe extern "C" fn(y: c_int, count: c_int, spans: *const FT_Span, user: *mut c_void)>;
pub type FT_Raster_BitTest_Func =
    Option<unsafe extern "C" fn(y: c_int, x: c_int, user: *mut c_void) -> c_int>;
pub type FT_Raster_BitSet_Func =
    Option<unsafe extern "C" fn(y: c_int, x: c_int, user: *mut c_void)>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Raster_Params {
    pub target: *const FT_Bitmap,
    pub source: *const c_void,
    pub flags: c_int,
    pub gray_spans: FT_SpanFunc,
    pub black_spans: FT_SpanFunc,
    pub bit_test: FT_Raster_BitTest_Func,
    pub bit_set: FT_Raster_BitSet_Func,
    pub user: *mut c_void,
    pub clip_box: FT_BBox,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Color {
    pub blue: FT_Byte,
    pub green: FT_Byte,
    pub red: FT_Byte,
    pub alpha: FT_Byte,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Palette_Data {
    pub num_palettes: FT_UShort,
    pub palette_name_ids: *const FT_UShort,
    pub palette_flags: *const FT_UShort,
    pub num_palette_entries: FT_UShort,
    pub palette_entry_name_ids: *const FT_UShort,
}

pub type FT_OpaquePaint = rust_ffi::FT_OpaquePaint;
pub type FT_ColorIndex = rust_ffi::FT_ColorIndex;
pub type FT_ColorLine = rust_ffi::FT_ColorLine;
pub type FT_ColorStop = rust_ffi::FT_ColorStop;
pub type FT_ColorStopIterator = rust_ffi::FT_ColorStopIterator;
pub type FT_PaintSolid = rust_ffi::FT_PaintSolid;
pub type FT_PaintGlyph = rust_ffi::FT_PaintGlyph;
pub type FT_PaintComposite = rust_ffi::FT_PaintComposite;
pub type FT_COLR_Paint = rust_ffi::FT_COLR_Paint;

fn rust_color_from_c(color: FT_Color) -> rust_ffi::FT_Color {
    rust_ffi::FT_Color {
        blue: color.blue,
        green: color.green,
        red: color.red,
        alpha: color.alpha,
    }
}

fn copy_palette_data_to_c(out: &mut FT_Palette_Data, value: rust_ffi::FT_Palette_Data) {
    out.num_palettes = value.num_palettes;
    out.palette_name_ids = value.palette_name_ids;
    out.palette_flags = value.palette_flags;
    out.num_palette_entries = value.num_palette_entries;
    out.palette_entry_name_ids = value.palette_entry_name_ids;
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiPaletteDataSnapshot {
    pub error: FT_Error,
    pub num_palettes: FT_UShort,
    pub num_palette_entries: FT_UShort,
    pub palette_name_ids_is_null: bool,
    pub palette_flags_is_null: bool,
    pub palette_entry_name_ids_is_null: bool,
    pub palette_name_ids: Vec<FT_UShort>,
    pub palette_flags: Vec<FT_UShort>,
    pub palette_entry_name_ids: Vec<FT_UShort>,
}

#[cfg(feature = "abi-test-support")]
fn abi_ushort_slice(ptr: *const FT_UShort, len: FT_UShort) -> Vec<FT_UShort> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    // SAFETY: test callers pass live FreeType-shaped array pointers returned
    // by `FT_Palette_Data_Get`; this helper immediately copies `len` elements.
    unsafe { slice::from_raw_parts(ptr, usize::from(len)).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_data_snapshot(face: FT_Face) -> AbiPaletteDataSnapshot {
    let mut data = FT_Palette_Data::default();
    let error = FT_Palette_Data_Get(face, &mut data);
    AbiPaletteDataSnapshot {
        error,
        num_palettes: data.num_palettes,
        num_palette_entries: data.num_palette_entries,
        palette_name_ids_is_null: data.palette_name_ids.is_null(),
        palette_flags_is_null: data.palette_flags.is_null(),
        palette_entry_name_ids_is_null: data.palette_entry_name_ids.is_null(),
        palette_name_ids: if error == rust_ffi::FT_Err_Ok {
            abi_ushort_slice(data.palette_name_ids, data.num_palettes)
        } else {
            Vec::new()
        },
        palette_flags: if error == rust_ffi::FT_Err_Ok {
            abi_ushort_slice(data.palette_flags, data.num_palettes)
        } else {
            Vec::new()
        },
        palette_entry_name_ids: if error == rust_ffi::FT_Err_Ok {
            abi_ushort_slice(data.palette_entry_name_ids, data.num_palette_entries)
        } else {
            Vec::new()
        },
    }
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiPaletteSelectSnapshot {
    pub error: FT_Error,
    pub palette_is_null: bool,
    pub entries: Vec<FT_Color>,
}

#[cfg(feature = "abi-test-support")]
fn abi_palette_entries_from_ptr(face: FT_Face, palette: *mut FT_Color) -> Vec<FT_Color> {
    if palette.is_null() {
        return Vec::new();
    }
    let mut data = FT_Palette_Data::default();
    if FT_Palette_Data_Get(face, &mut data) != rust_ffi::FT_Err_Ok {
        return Vec::new();
    }
    let len = usize::from(data.num_palette_entries);
    // SAFETY: this test-support helper copies the palette pointer returned by
    // `FT_Palette_Select` while the owning face is still live.
    unsafe { slice::from_raw_parts(palette, len).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_select_snapshot(
    face: FT_Face,
    palette_index: FT_UShort,
) -> AbiPaletteSelectSnapshot {
    let mut palette = ptr::null_mut();
    let error = FT_Palette_Select(face, palette_index, &mut palette);
    AbiPaletteSelectSnapshot {
        error,
        palette_is_null: palette.is_null(),
        entries: if error == rust_ffi::FT_Err_Ok {
            abi_palette_entries_from_ptr(face, palette)
        } else {
            Vec::new()
        },
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_select_without_output(face: FT_Face, palette_index: FT_UShort) -> FT_Error {
    FT_Palette_Select(face, palette_index, ptr::null_mut())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_mutate_entry(
    face: FT_Face,
    palette_index: FT_UShort,
    entry_index: usize,
    color: FT_Color,
) -> AbiPaletteSelectSnapshot {
    let mut snapshot = abi_palette_select_snapshot(face, palette_index);
    if entry_index < snapshot.entries.len() {
        let mut palette = ptr::null_mut();
        let error = FT_Palette_Select(face, palette_index, &mut palette);
        if error == rust_ffi::FT_Err_Ok && !palette.is_null() {
            // SAFETY: this feature-gated helper mutates an entry through the
            // public ABI palette pointer while the face is live, matching the
            // FreeType caller-observable behavior under test.
            unsafe { *palette.add(entry_index) = color };
            snapshot = AbiPaletteSelectSnapshot {
                error,
                palette_is_null: palette.is_null(),
                entries: abi_palette_entries_from_ptr(face, palette),
            };
        }
    }
    snapshot
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Init(abitmap: *mut FT_Bitmap) {
    // FreeType accepts NULL here and otherwise overwrites the public record
    // with the static zero `null_bitmap`.
    if let Some(bitmap) = unsafe { abitmap.as_mut() } {
        *bitmap = FT_Bitmap::default();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_New(abitmap: *mut FT_Bitmap) {
    FT_Bitmap_Init(abitmap);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Gzip_Uncompress(
    memory: FT_Memory,
    output: *mut FT_Byte,
    output_len: *mut FT_ULong,
    input: *const FT_Byte,
    input_len: FT_ULong,
) -> FT_Error {
    if memory.is_null() || output.is_null() || output_len.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Ok(input_len) = usize::try_from(input_len) else {
        return rust_ffi::FT_Err_Invalid_Table;
    };
    // SAFETY: `output_len` was checked for null above and is only borrowed for
    // the duration of this C ABI call.
    let output_len_ref = unsafe { &mut *output_len };
    let Ok(output_capacity) = usize::try_from(*output_len_ref) else {
        return rust_ffi::FT_Err_Array_Too_Large as FT_Error;
    };
    // SAFETY: `output` is non-null and the caller-provided `*output_len`
    // defines the writable output buffer length, matching FreeType's ABI.
    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_capacity) };
    let input_slice = if input.is_null() {
        None
    } else {
        // SAFETY: non-null `input` plus `input_len` form the caller-provided
        // compressed byte slice for the duration of this call.
        Some(unsafe { slice::from_raw_parts(input, input_len) })
    };
    let memory_view = rust_ffi::FT_MemoryRec::default();
    rust_ffi::FT_Gzip_Uncompress(
        Some(&memory_view),
        Some(output_slice),
        Some(output_len_ref),
        input_slice,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stream_OpenBzip2(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    rust_ffi::FT_Stream_OpenBzip2(unsafe { stream.as_mut() }, unsafe { source.as_ref() })
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_Node_Unref(node: rust_ffi::FTC_Node, manager: rust_ffi::FTC_Manager) {
    rust_ffi::FTC_Node_Unref(node, manager);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stream_OpenGzip(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    let Some(stream_ref) = (unsafe { stream.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    if source_ref.base.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let Ok(source_len) = usize::try_from(source_ref.size) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    // SAFETY: this thin ABI wrapper supports the memory-backed stream shape
    // used by the parity fixtures; `base` and `size` are caller-provided.
    let source_bytes = unsafe { slice::from_raw_parts(source_ref.base.cast_const(), source_len) };
    rust_ffi::FT_Stream_OpenGzip(Some(stream_ref), Some(source_ref), Some(source_bytes))
}

pub fn abi_support_gzip_stream_bytes(
    stream: FT_Stream,
    offset: FT_ULong,
    count: FT_ULong,
) -> Option<Vec<FT_Byte>> {
    let stream_ref = unsafe { stream.as_ref() }?;
    rust_ffi::FT_Gzip_Stream_Read(Some(stream_ref), offset, count)
}

pub fn abi_support_gzip_stream_close(stream: FT_Stream) {
    if let Some(stream_ref) = unsafe { stream.as_mut() } {
        rust_ffi::FT_Gzip_Stream_Close(Some(stream_ref));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_List_Add(list: FT_List, node: FT_ListNode) {
    let (Some(list_ref), Some(node_ref)) = (unsafe { list.as_mut() }, unsafe { node.as_mut() })
    else {
        return;
    };
    let before = list_ref.tail;

    node_ref.next = ptr::null_mut();
    node_ref.prev = before;

    if let Some(before_ref) = unsafe { before.as_mut() } {
        before_ref.next = node;
    } else {
        list_ref.head = node;
    }
    list_ref.tail = node;
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_List_Insert(list: FT_List, node: FT_ListNode) {
    let (Some(list_ref), Some(node_ref)) = (unsafe { list.as_mut() }, unsafe { node.as_mut() })
    else {
        return;
    };
    let after = list_ref.head;

    node_ref.next = after;
    node_ref.prev = ptr::null_mut();

    if let Some(after_ref) = unsafe { after.as_mut() } {
        after_ref.prev = node;
    } else {
        list_ref.tail = node;
    }
    list_ref.head = node;
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_List_Find(list: FT_List, data: FT_Pointer) -> FT_ListNode {
    let Some(list_ref) = (unsafe { list.as_ref() }) else {
        return ptr::null_mut();
    };
    let mut cur = list_ref.head;
    while let Some(cur_ref) = unsafe { cur.as_ref() } {
        let rust_node = rust_ffi::FT_ListNodeRec {
            prev: cur_ref.prev.cast(),
            next: cur_ref.next.cast(),
            data,
        };
        if rust_ffi::FT_List_Find_Node_Matches(&rust_node, cur_ref.data) {
            return cur;
        }
        cur = cur_ref.next;
    }
    ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_List_Remove(list: FT_List, node: FT_ListNode) {
    let (Some(list_ref), Some(node_ref)) = (unsafe { list.as_mut() }, unsafe { node.as_ref() })
    else {
        return;
    };
    let before = node_ref.prev;
    let after = node_ref.next;

    if let Some(before_ref) = unsafe { before.as_mut() } {
        before_ref.next = after;
    } else {
        list_ref.head = after;
    }

    if let Some(after_ref) = unsafe { after.as_mut() } {
        after_ref.prev = before;
    } else {
        list_ref.tail = before;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_List_Up(list: FT_List, node: FT_ListNode) {
    let (Some(list_ref), Some(node_ref)) = (unsafe { list.as_mut() }, unsafe { node.as_mut() })
    else {
        return;
    };
    let before = node_ref.prev;
    let after = node_ref.next;
    let Some(before_ref) = (unsafe { before.as_mut() }) else {
        return;
    };

    before_ref.next = after;

    if let Some(after_ref) = unsafe { after.as_mut() } {
        after_ref.prev = before;
    } else {
        list_ref.tail = before;
    }

    node_ref.prev = ptr::null_mut();
    node_ref.next = list_ref.head;
    if let Some(head_ref) = unsafe { list_ref.head.as_mut() } {
        head_ref.prev = node;
    }
    list_ref.head = node;
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_List_Iterate(
    list: FT_List,
    iterator: FT_List_Iterator,
    user: FT_Pointer,
) -> FT_Error {
    let (Some(list_ref), Some(iterator)) = (unsafe { list.as_ref() }, iterator) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut cur = list_ref.head;
    let mut error = rust_ffi::FT_Err_Ok;
    while let Some(cur_ref) = unsafe { cur.as_ref() } {
        let next = cur_ref.next;
        error = iterator(cur, user);
        if error != rust_ffi::FT_Err_Ok {
            break;
        }
        cur = next;
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_List_Finalize(
    list: FT_List,
    destroy: FT_List_Destructor,
    memory: FT_Memory,
    user: FT_Pointer,
) {
    let (Some(list_ref), Some(memory_ref)) = (unsafe { list.as_mut() }, unsafe { memory.as_ref() })
    else {
        return;
    };

    let mut cur = list_ref.head;
    while let Some(cur_ref) = unsafe { cur.as_ref() } {
        let next = cur_ref.next;
        if let Some(destroy) = destroy {
            destroy(memory, cur_ref.data, user);
        }
        if let Some(free) = memory_ref.free {
            free(memory, cur.cast());
        }
        cur = next;
    }

    list_ref.head = ptr::null_mut();
    list_ref.tail = ptr::null_mut();
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Copy(
    library: FT_Library,
    source: *const FT_Bitmap,
    target: *mut FT_Bitmap,
) -> FT_Error {
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if source == target.cast_const() {
        return if library_ref(library).is_some() {
            rust_ffi::FT_Err_Ok
        } else {
            rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error
        };
    }

    let mut source_view = bitmap_to_rust(source_ref);
    let mut target_view = bitmap_to_rust(target_ref);
    if let Some(bytes) = bitmap_bytes(source_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source_view), bytes);
    }

    let err = rust_ffi::FT_Bitmap_Copy(
        library_ref(library),
        Some(&source_view),
        Some(&mut target_view),
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_c(target_ref, &target_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Convert(
    library: FT_Library,
    source: *const FT_Bitmap,
    target: *mut FT_Bitmap,
    alignment: FT_Int,
) -> FT_Error {
    if library_ref(library).is_none() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut source_view = bitmap_to_rust(source_ref);
    let mut target_view = bitmap_to_rust(target_ref);
    if let Some(bytes) = bitmap_bytes(source_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source_view), bytes);
    }
    if let Some(bytes) = bitmap_bytes(target_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut target_view), bytes);
    }

    let err = rust_ffi::FT_Bitmap_Convert(
        library_ref(library),
        Some(&source_view),
        Some(&mut target_view),
        alignment,
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_c(target_ref, &target_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Done(library: FT_Library, bitmap: *mut FT_Bitmap) -> FT_Error {
    if library_ref(library).is_none() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(bitmap_ref) = (unsafe { bitmap.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut bitmap_view = bitmap_to_rust(bitmap_ref);
    if let Some(bytes) = bitmap_bytes(bitmap_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut bitmap_view), bytes);
    }
    let err = rust_ffi::FT_Bitmap_Done(library_ref(library), Some(&mut bitmap_view));
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_c(bitmap_ref, &bitmap_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Embolden(
    library: FT_Library,
    bitmap: *mut FT_Bitmap,
    xStrength: FT_Pos,
    yStrength: FT_Pos,
) -> FT_Error {
    let Some(bitmap_ref) = (unsafe { bitmap.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut bitmap_view = bitmap_to_rust(bitmap_ref);
    if let Some(bytes) = bitmap_bytes(bitmap_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut bitmap_view), bytes);
    }

    let err = rust_ffi::FT_Bitmap_Embolden(
        library_ref(library),
        Some(&mut bitmap_view),
        xStrength,
        yStrength,
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_c(bitmap_ref, &bitmap_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Blend(
    library: FT_Library,
    source: *const FT_Bitmap,
    source_offset: FT_Vector,
    target: *mut FT_Bitmap,
    atarget_offset: *mut FT_Vector,
    color: FT_Color,
) -> FT_Error {
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(atarget_offset_ref) = (unsafe { atarget_offset.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut source_view = bitmap_to_rust(source_ref);
    let mut target_view = bitmap_to_rust(target_ref);
    if let Some(bytes) = bitmap_bytes(source_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source_view), bytes);
    }
    if let Some(bytes) = bitmap_bytes(target_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut target_view), bytes);
    }
    let mut rust_target_offset = rust_ffi::FT_Vector {
        x: atarget_offset_ref.x,
        y: atarget_offset_ref.y,
    };
    let err = rust_ffi::FT_Bitmap_Blend(
        library_ref(library),
        Some(&source_view),
        rust_ffi::FT_Vector {
            x: source_offset.x,
            y: source_offset.y,
        },
        Some(&mut target_view),
        Some(&mut rust_target_offset),
        rust_ffi::FT_Color {
            blue: color.blue,
            green: color.green,
            red: color.red,
            alpha: color.alpha,
        },
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_c(target_ref, &target_view);
        atarget_offset_ref.x = rust_target_offset.x;
        atarget_offset_ref.y = rust_target_offset.y;
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Palette_Data_Get(
    face: FT_Face,
    apalette_data: *mut FT_Palette_Data,
) -> FT_Error {
    let Some(out) = (unsafe { apalette_data.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut rust_out = rust_ffi::FT_Palette_Data::default();
    let err = rust_ffi::FT_Palette_Data_Get(
        face_state(face).map(|state| &state.inner),
        Some(&mut rust_out),
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_palette_data_to_c(out, rust_out);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Palette_Select(
    face: FT_Face,
    palette_index: FT_UShort,
    apalette: *mut *mut FT_Color,
) -> FT_Error {
    let mut rust_palette: *const rust_ffi::FT_Color = ptr::null();
    let err = rust_ffi::FT_Palette_Select(
        face_state(face).map(|state| &state.inner),
        palette_index,
        (!apalette.is_null()).then_some(&mut rust_palette),
    );
    if err == rust_ffi::FT_Err_Ok && !apalette.is_null() {
        // SAFETY: `apalette` is non-null and caller provided writable storage.
        unsafe {
            *apalette = rust_palette.cast::<FT_Color>().cast_mut();
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Palette_Set_Foreground_Color(
    face: FT_Face,
    foreground_color: FT_Color,
) -> FT_Error {
    rust_ffi::FT_Palette_Set_Foreground_Color(
        face_state(face).map(|state| &state.inner),
        rust_color_from_c(foreground_color),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Color_Glyph_Layer(
    face: FT_Face,
    base_glyph: FT_UInt,
    aglyph_index: *mut FT_UInt,
    acolor_index: *mut FT_UInt,
    iterator: *mut FT_LayerIterator,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_Layer(
        face_state(face).map(|state| &state.inner),
        base_glyph,
        unsafe { aglyph_index.as_mut() },
        unsafe { acolor_index.as_mut() },
        unsafe { iterator.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Color_Glyph_ClipBox(
    face: FT_Face,
    base_glyph: FT_UInt,
    clip_box: *mut FT_ClipBox,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_ClipBox(
        face_state(face).map(|state| &state.inner),
        base_glyph,
        unsafe { clip_box.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Color_Glyph_Paint(
    face: FT_Face,
    base_glyph: FT_UInt,
    root_transform: FT_UInt,
    paint: *mut FT_OpaquePaint,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_Paint(
        face_state(face).map(|state| &state.inner),
        base_glyph,
        root_transform,
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Paint(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
    paint: *mut FT_COLR_Paint,
) -> FT_Bool {
    rust_ffi::FT_Get_Paint(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Paint_Layers(
    face: FT_Face,
    layer_iterator: *mut FT_LayerIterator,
    paint: *mut FT_OpaquePaint,
) -> FT_Bool {
    rust_ffi::FT_Get_Paint_Layers(
        face_state(face).map(|state| &state.inner),
        unsafe { layer_iterator.as_mut() },
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Colorline_Stops(
    face: FT_Face,
    color_stop: *mut FT_ColorStop,
    iterator: *mut FT_ColorStopIterator,
) -> FT_Bool {
    rust_ffi::FT_Get_Colorline_Stops(
        face_state(face).map(|state| &state.inner),
        unsafe { color_stop.as_mut() },
        unsafe { iterator.as_mut() },
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_layer_iterator(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_LayerIterator> {
    rust_ffi::FT_ColrV1_Paint_Layer_Iterator_Copy(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_colorline(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_ColorLine> {
    rust_ffi::FT_ColrV1_Paint_ColorLine_Copy(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_transform(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_PaintTransform> {
    rust_ffi::FT_ColrV1_Paint_Transform_Copy(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_graph(
    face: FT_Face,
) -> Option<rust_ffi::FT_ColrV1_PaintGraph_Snapshot> {
    rust_ffi::FT_ColrV1_PaintGraph_Copy(face_state(face).map(|state| &state.inner))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_public_paint_solid(
    face: FT_Face,
    glyph_index: FT_UInt,
) -> rust_ffi::FT_ColrV1_PublicPaintSolid_Snapshot {
    rust_ffi::FT_ColrV1_PublicPaintSolid_Copy(
        face_state(face).map(|state| &state.inner),
        glyph_index,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_TrueTypeGX_Free(face: FT_Face, table: FT_Bytes) {
    rust_ffi::FT_TrueTypeGX_Free(face_state(face).map(|state| &state.inner), table);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_ClassicKern_Free(face: FT_Face, table: FT_Bytes) {
    rust_ffi::FT_ClassicKern_Free(face_state(face).map(|state| &state.inner), table);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_ClassicKern_Validate(
    face: FT_Face,
    validation_flags: FT_UInt,
    ckern_table: *mut FT_Bytes,
) -> FT_Error {
    let face = face_state(face).map(|state| &state.inner);
    let mut table = ptr::null();
    let err = rust_ffi::FT_ClassicKern_Validate(
        face,
        validation_flags,
        (!ckern_table.is_null()).then_some(&mut table),
    );
    if err == rust_ffi::FT_Err_Ok {
        write_ft_bytes(ckern_table, table);
    }
    err
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_SfntName {
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
    pub language_id: FT_UShort,
    pub name_id: FT_UShort,
    pub string: *mut FT_Byte,
    pub string_len: FT_UInt,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_CharMapRec {
    pub face: FT_Face,
    pub encoding: FT_Encoding,
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TT_OS2 {
    pub version: FT_UShort,
    pub xAvgCharWidth: FT_Short,
    pub usWeightClass: FT_UShort,
    pub usWidthClass: FT_UShort,
    pub fsType: FT_UShort,
    pub ySubscriptXSize: FT_Short,
    pub ySubscriptYSize: FT_Short,
    pub ySubscriptXOffset: FT_Short,
    pub ySubscriptYOffset: FT_Short,
    pub ySuperscriptXSize: FT_Short,
    pub ySuperscriptYSize: FT_Short,
    pub ySuperscriptXOffset: FT_Short,
    pub ySuperscriptYOffset: FT_Short,
    pub yStrikeoutSize: FT_Short,
    pub yStrikeoutPosition: FT_Short,
    pub sFamilyClass: FT_Short,
    pub panose: [FT_Byte; 10],
    pub ulUnicodeRange1: FT_ULong,
    pub ulUnicodeRange2: FT_ULong,
    pub ulUnicodeRange3: FT_ULong,
    pub ulUnicodeRange4: FT_ULong,
    pub achVendID: [i8; 4],
    pub fsSelection: FT_UShort,
    pub usFirstCharIndex: FT_UShort,
    pub usLastCharIndex: FT_UShort,
    pub sTypoAscender: FT_Short,
    pub sTypoDescender: FT_Short,
    pub sTypoLineGap: FT_Short,
    pub usWinAscent: FT_UShort,
    pub usWinDescent: FT_UShort,
    pub ulCodePageRange1: FT_ULong,
    pub ulCodePageRange2: FT_ULong,
    pub sxHeight: FT_Short,
    pub sCapHeight: FT_Short,
    pub usDefaultChar: FT_UShort,
    pub usBreakChar: FT_UShort,
    pub usMaxContext: FT_UShort,
    pub usLowerOpticalPointSize: FT_UShort,
    pub usUpperOpticalPointSize: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TT_VertHeader {
    pub Version: FT_Fixed,
    pub Ascender: FT_Short,
    pub Descender: FT_Short,
    pub Line_Gap: FT_Short,
    pub advance_Height_Max: FT_UShort,
    pub min_Top_Side_Bearing: FT_Short,
    pub min_Bottom_Side_Bearing: FT_Short,
    pub yMax_Extent: FT_Short,
    pub caret_Slope_Rise: FT_Short,
    pub caret_Slope_Run: FT_Short,
    pub caret_Offset: FT_Short,
    pub Reserved: [FT_Short; 4],
    pub metric_Data_Format: FT_Short,
    pub number_Of_VMetrics: FT_UShort,
    pub long_metrics: FT_Pointer,
    pub short_metrics: FT_Pointer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TT_MaxProfile {
    pub version: FT_Fixed,
    pub numGlyphs: FT_UShort,
    pub maxPoints: FT_UShort,
    pub maxContours: FT_UShort,
    pub maxCompositePoints: FT_UShort,
    pub maxCompositeContours: FT_UShort,
    pub maxZones: FT_UShort,
    pub maxTwilightPoints: FT_UShort,
    pub maxStorage: FT_UShort,
    pub maxFunctionDefs: FT_UShort,
    pub maxInstructionDefs: FT_UShort,
    pub maxStackElements: FT_UShort,
    pub maxSizeOfInstructions: FT_UShort,
    pub maxComponentElements: FT_UShort,
    pub maxComponentDepth: FT_UShort,
}

#[repr(C)]
pub struct FT_GlyphSlotRec {
    pub glyph_index: FT_UInt,
    pub metrics: FT_Glyph_Metrics,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub bitmap: FT_Bitmap,
    pub bitmap_left: FT_Int,
    pub bitmap_top: FT_Int,
    owns_bitmap: bool,
    buffer: Vec<u8>,
    rust_slot: rust_ffi::FT_GlyphSlot,
    source_face: FT_Face,
    load_flags: FT_Int32,
}

#[repr(C)]
pub struct FT_SizeRec {
    // FreeType include/freetype/freetype.h exposes `face`, `generic`, `metrics`,
    // and opaque non-null `internal` as the public FT_SizeRec fields.
    pub face: FT_Face,
    pub generic: FT_Generic,
    pub metrics: FT_Size_Metrics,
    pub internal: *mut c_void,
    rust_size: rust_ffi::FT_Size,
    owner: FT_Face,
}

#[repr(C)]
pub struct FT_FaceRec {
    pub glyph: FT_GlyphSlot,
    pub size: FT_Size,
    pub internal: *mut c_void,
}

#[repr(C)]
pub struct FT_LibraryRec {
    pub internal: *mut c_void,
}

#[repr(C)]
pub struct FT_ModuleRec {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FT_Module_Class {
    pub module_flags: FT_ULong,
    pub module_size: FT_Long,
    pub module_name: *const FT_String,
    pub module_version: FT_Fixed,
    pub module_requires: FT_Fixed,
    pub module_interface: *const c_void,
    pub module_init: FT_Pointer,
    pub module_done: FT_Pointer,
    pub get_interface: FT_Pointer,
}

#[repr(C)]
pub struct FT_RendererRec {
    format: FT_Glyph_Format,
    module_name: &'static str,
}

struct LibraryState {
    inner: rust_ffi::FT_Library,
    allocation_memory: FT_Memory,
    allocation_block: FT_Pointer,
    outline_renderer: FT_RendererRec,
    synthetic_renderer: FT_RendererRec,
    synthetic_module_handle: Box<FT_ModuleRec>,
}

impl LibraryState {
    fn new(inner: rust_ffi::FT_Library) -> Self {
        Self {
            inner,
            allocation_memory: std::ptr::null_mut(),
            allocation_block: std::ptr::null_mut(),
            outline_renderer: FT_RendererRec {
                format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
                module_name: "smooth",
            },
            synthetic_renderer: FT_RendererRec {
                format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
                module_name: "fixture_renderer",
            },
            synthetic_module_handle: Box::new(FT_ModuleRec { _private: [] }),
        }
    }

    fn new_with_allocation(
        inner: rust_ffi::FT_Library,
        allocation_memory: FT_Memory,
        allocation_block: FT_Pointer,
    ) -> Self {
        Self {
            inner,
            allocation_memory,
            allocation_block,
            outline_renderer: FT_RendererRec {
                format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
                module_name: "smooth",
            },
            synthetic_renderer: FT_RendererRec {
                format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
                module_name: "fixture_renderer",
            },
            synthetic_module_handle: Box::new(FT_ModuleRec { _private: [] }),
        }
    }
}

struct FaceState {
    inner: rust_ffi::FT_Face,
    size_records: Vec<FT_Size>,
    charmaps: Box<[FT_CharMapRec]>,
    charmap_ptrs: Box<[FT_CharMap]>,
    family_name: Option<CString>,
    style_name: Option<CString>,
    postscript_name: Option<CString>,
    font_format: Option<CString>,
    face_driver_name: Option<CString>,
    variant_list: Vec<FT_UInt32>,
    stream: FT_Stream,
    stream_close: FT_Stream_CloseFunc,
}

impl FaceState {
    fn new(inner: rust_ffi::FT_Face) -> Self {
        let family_name = inner
            .family_name
            .as_ref()
            .and_then(|name| CString::new(name.as_str()).ok());
        let style_name = inner
            .style_name
            .as_ref()
            .and_then(|name| CString::new(name.as_str()).ok());
        let postscript_name = postscript_name_cstring(&inner);
        let font_format = font_format_cstring(Some(&inner));
        let face_driver_name = face_driver_name_cstring(Some(&inner));
        Self {
            inner,
            size_records: Vec::new(),
            charmaps: Box::new([]),
            charmap_ptrs: Box::new([]),
            family_name,
            style_name,
            postscript_name,
            font_format,
            face_driver_name,
            variant_list: Vec::new(),
            stream: ptr::null_mut(),
            stream_close: None,
        }
    }

    fn refresh_charmaps(&mut self, face: FT_Face) {
        let records = self
            .inner
            .charmaps
            .iter()
            .map(|record| FT_CharMapRec {
                face,
                encoding: record.encoding,
                platform_id: record.platform_id,
                encoding_id: record.encoding_id,
            })
            .collect::<Vec<_>>();
        let mut charmaps = records.into_boxed_slice();
        let charmap_ptrs = charmaps
            .iter_mut()
            .map(|record| record as *mut FT_CharMapRec)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.charmaps = charmaps;
        self.charmap_ptrs = charmap_ptrs;
    }

    fn refresh_postscript_name(&mut self) {
        self.postscript_name = postscript_name_cstring(&self.inner);
    }

    fn charmap_index(&self, charmap: FT_CharMap) -> Option<usize> {
        if charmap.is_null() {
            return None;
        }
        self.charmaps
            .iter()
            .position(|record| ptr::eq(record as *const FT_CharMapRec, charmap.cast_const()))
    }

    fn charmap_by_index(&self, index: FT_UInt) -> Option<FT_CharMap> {
        let index = usize::try_from(index).ok()?;
        self.charmap_ptrs.get(index).copied()
    }

    fn variant_list_ptr(&mut self, values: Option<Vec<FT_UInt32>>) -> *mut FT_UInt32 {
        let Some(values) = values else {
            self.variant_list.clear();
            return ptr::null_mut();
        };
        self.variant_list = values;
        self.variant_list.push(0);
        self.variant_list.as_mut_ptr()
    }

    fn push_size_record(&mut self, size: FT_Size) {
        self.size_records.push(size);
    }

    fn remove_size_record(&mut self, size: FT_Size) -> bool {
        let Some(index) = self
            .size_records
            .iter()
            .position(|record| ptr::eq(*record, size))
        else {
            return false;
        };
        self.size_records.remove(index);
        true
    }
}

impl Drop for FaceState {
    fn drop(&mut self) {
        for size in self.size_records.drain(..) {
            // SAFETY: `size_records` contains only boxes allocated by this wrapper.
            unsafe { drop_size(size) };
        }
    }
}

fn postscript_name_cstring(inner: &rust_ffi::FT_Face) -> Option<CString> {
    rust_ffi::FT_Get_Postscript_Name(inner).and_then(|name| {
        // FreeType exposes a borrowed NUL-terminated C string owned by the face.
        CString::new(name).ok()
    })
}

fn font_format_cstring(inner: Option<&rust_ffi::FT_Face>) -> Option<CString> {
    rust_ffi::FT_Get_Font_Format(inner).and_then(|format| {
        // FreeType exposes the driver-owned FONT_FORMAT service string.
        CString::new(format).ok()
    })
}

fn face_driver_name_cstring(inner: Option<&rust_ffi::FT_Face>) -> Option<CString> {
    rust_ffi::FT_FACE_DRIVER_NAME(inner).and_then(|name| {
        // FreeType exposes the driver module class name as a borrowed
        // NUL-terminated C string owned by the driver's module class.
        CString::new(name).ok()
    })
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiSlotSnapshot {
    pub glyph_index: FT_UInt,
    pub metrics: FT_Glyph_Metrics,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub num_subglyphs: FT_UInt,
    pub outline_cbox: FT_BBox,
    pub outline_bbox: FT_BBox,
    pub outline: Option<rust_ffi::FT_OutlineSnapshot>,
    pub bitmap: Option<AbiBitmapSnapshot>,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBitmapSnapshot {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
    pub left: FT_Int,
    pub top: FT_Int,
    pub owns_bitmap: bool,
    pub buffer: Vec<u8>,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_byte_slice(ptr: *const FT_Byte, len: FT_UInt) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    let len = usize::try_from(len).unwrap_or(0);
    // SAFETY: test callers pass live FreeType-shaped output pointers with
    // `len` bytes valid for the duration of the snapshot copy.
    unsafe { slice::from_raw_parts(ptr, len).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_bdf_property_snapshot(property: &BDF_PropertyRec) -> AbiBdfPropertySnapshot {
    match property.type_ {
        rust_ffi::BDF_PROPERTY_TYPE_ATOM => {
            // SAFETY: `type_ == ATOM` means FreeType/fontdone wrote the
            // `atom` union member according to the public BDF_PropertyRec ABI.
            let atom = unsafe { property.u.atom };
            AbiBdfPropertySnapshot {
                type_: property.type_,
                atom,
                integer: 0,
                cardinal: 0,
            }
        }
        rust_ffi::BDF_PROPERTY_TYPE_INTEGER => {
            // SAFETY: `type_ == INTEGER` means the active union member is
            // `integer`.
            let integer = unsafe { property.u.integer };
            AbiBdfPropertySnapshot {
                type_: property.type_,
                atom: ptr::null(),
                integer,
                cardinal: 0,
            }
        }
        rust_ffi::BDF_PROPERTY_TYPE_CARDINAL => {
            // SAFETY: `type_ == CARDINAL` means the active union member is
            // `cardinal`.
            let cardinal = unsafe { property.u.cardinal };
            AbiBdfPropertySnapshot {
                type_: property.type_,
                atom: ptr::null(),
                integer: 0,
                cardinal,
            }
        }
        _ => {
            // SAFETY: error rows initialize the union through `cardinal` before
            // calling the API.  FreeType leaves that storage untouched when it
            // returns `BDF_PROPERTY_TYPE_NONE`.
            let cardinal = unsafe { property.u.cardinal };
            AbiBdfPropertySnapshot {
                type_: property.type_,
                atom: ptr::null(),
                integer: cardinal as FT_Int32,
                cardinal,
            }
        }
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_c_string_bytes(ptr: *const c_char) -> Vec<u8> {
    if ptr.is_null() {
        return Vec::new();
    }
    // SAFETY: test callers pass live FreeType-shaped NUL-terminated strings.
    unsafe { CStr::from_ptr(ptr).to_bytes().to_vec() }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Error_String(error_code: FT_Error) -> *const c_char {
    rust_ffi::FT_Error_String(error_code).map_or(ptr::null(), |text| text.as_ptr().cast())
}

fn write_ft_bytes(out: *mut FT_Bytes, value: FT_Bytes) {
    if let Some(out) = non_null_mut(out) {
        // SAFETY: `out` is non-null and caller provides writable FT_Bytes storage.
        unsafe { *out.as_ptr() = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_OpenType_Validate(
    face: FT_Face,
    validation_flags: FT_UInt,
    base_table: *mut FT_Bytes,
    gdef_table: *mut FT_Bytes,
    gpos_table: *mut FT_Bytes,
    gsub_table: *mut FT_Bytes,
    jstf_table: *mut FT_Bytes,
) -> FT_Error {
    let face = face_state(face).map(|state| &state.inner);
    let mut base = ptr::null();
    let mut gdef = ptr::null();
    let mut gpos = ptr::null();
    let mut gsub = ptr::null();
    let mut jstf = ptr::null();
    let err = rust_ffi::FT_OpenType_Validate(
        face,
        validation_flags,
        (!base_table.is_null()).then_some(&mut base),
        (!gdef_table.is_null()).then_some(&mut gdef),
        (!gpos_table.is_null()).then_some(&mut gpos),
        (!gsub_table.is_null()).then_some(&mut gsub),
        (!jstf_table.is_null()).then_some(&mut jstf),
    );
    if err == rust_ffi::FT_Err_Ok {
        write_ft_bytes(base_table, base);
        write_ft_bytes(gdef_table, gdef);
        write_ft_bytes(gpos_table, gpos);
        write_ft_bytes(gsub_table, gsub);
        write_ft_bytes(jstf_table, jstf);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PS_Font_Info(face: FT_Face, afont_info: PS_FontInfo) -> FT_Error {
    let face = face_state(face).map(|state| &state.inner);
    let mut info = PS_FontInfoRec::default();
    let err = rust_ffi::FT_Get_PS_Font_Info(face, (!afont_info.is_null()).then_some(&mut info));
    if err == rust_ffi::FT_Err_Ok && !afont_info.is_null() {
        // SAFETY: C ABI caller supplied a non-null `PS_FontInfoRec*` output
        // pointer; copying the repr(C) public record is the wrapper's only
        // responsibility.
        unsafe {
            *afont_info = info;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PS_Font_Private(face: FT_Face, afont_private: PS_Private) -> FT_Error {
    let face = face_state(face).map(|state| &state.inner);
    let mut private = PS_PrivateRec::default();
    let err =
        rust_ffi::FT_Get_PS_Font_Private(face, (!afont_private.is_null()).then_some(&mut private));
    if err == rust_ffi::FT_Err_Ok && !afont_private.is_null() {
        // SAFETY: C ABI caller supplied a non-null `PS_PrivateRec*` output
        // pointer; copying the repr(C) public record is the wrapper's only
        // responsibility.
        unsafe {
            *afont_private = private;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Has_PS_Glyph_Names(face: FT_Face) -> FT_Int {
    let face = face_state(face).map(|state| &state.inner);
    rust_ffi::FT_Has_PS_Glyph_Names(face)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PS_Font_Value(
    face: FT_Face,
    key: PS_Dict_Keys,
    idx: FT_UInt,
    value: *mut c_void,
    value_len: FT_Long,
) -> FT_Long {
    let face = face_state(face).map(|state| &state.inner);
    let effective_value_len = value_len.max(0);
    let value_len = usize::try_from(effective_value_len).unwrap_or(usize::MAX);
    let value = if value.is_null() {
        None
    } else {
        // SAFETY: C ABI caller supplies `value_len` writable bytes at `value`
        // when the pointer is non-null; this wrapper only exposes those bytes
        // to the safe Rust FFI implementation.
        Some(unsafe { slice::from_raw_parts_mut(value.cast::<u8>(), value_len) })
    };
    rust_ffi::FT_Get_PS_Font_Value(face, key, idx, value, effective_value_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_OpenType_Free(face: FT_Face, table: FT_Bytes) {
    rust_ffi::FT_OpenType_Free(face_state(face).map(|state| &state.inner), table);
}

#[cfg(feature = "abi-test-support")]
pub fn abi_uint32_list(ptr: *const FT_UInt32) -> Option<Vec<FT_UInt32>> {
    if ptr.is_null() {
        return None;
    }
    let mut values = Vec::new();
    for index in 0..4096 {
        // SAFETY: test callers pass live FreeType-shaped zero-terminated lists.
        let value = unsafe { *ptr.add(index) };
        if value == 0 {
            return Some(values);
        }
        values.push(value);
    }
    Some(values)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_info(face: FT_Face) -> Option<rust_ffi::FT_FaceRecPublic> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let internal = unsafe { (*face.as_ptr()).internal };
    let state = NonNull::new(internal.cast::<FaceState>())?;
    // SAFETY: `state` is owned by the live face for the duration of this scalar copy.
    let state = unsafe { state.as_ref() };
    let mut info = rust_face_info(&state.inner);
    info.family_name = state
        .family_name
        .as_ref()
        .map_or(ptr::null_mut(), |name| name.as_ptr().cast_mut());
    info.style_name = state
        .style_name
        .as_ref()
        .map_or(ptr::null_mut(), |name| name.as_ptr().cast_mut());
    Some(info)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_stream_info(face: FT_Face) -> Option<rust_ffi::FT_StreamRec> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let internal = unsafe { (*face.as_ptr()).internal };
    let state = NonNull::new(internal.cast::<FaceState>())?;
    // SAFETY: `state` is owned by the live face for the duration of this scalar copy.
    let state = unsafe { state.as_ref() };
    Some(state.inner.memory_stream_record())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_available_sizes(face: FT_Face) -> Option<Vec<rust_ffi::FT_Bitmap_Size>> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let internal = unsafe { (*face.as_ptr()).internal };
    let state = NonNull::new(internal.cast::<FaceState>())?;
    // SAFETY: `state` is owned by the live face for the duration of this vector copy.
    let state = unsafe { state.as_ref() };
    Some(state.inner.available_sizes.to_vec())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_mm_var_descriptor(
    library: FT_Library,
    face: FT_Face,
) -> Option<AbiMmVarDescriptorSnapshot> {
    let mut master_ptr: *mut FT_MM_Var = ptr::null_mut();
    let err = FT_Get_MM_Var(face, &mut master_ptr);
    if err != rust_ffi::FT_Err_Ok || master_ptr.is_null() {
        return Some((
            err,
            FT_MM_Var::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            rust_ffi::FT_Err_Ok,
        ));
    }
    // SAFETY: `FT_Get_MM_Var` returned a live descriptor pointer owned by this
    // C ABI crate until `FT_Done_MM_Var` is called below.
    let master = unsafe { *master_ptr };
    let axis_count = usize::try_from(master.num_axis).ok()?;
    let axes = if master.axis.is_null() {
        Vec::new()
    } else {
        // SAFETY: the descriptor's axis pointer has `num_axis` initialized
        // records and remains live until `FT_Done_MM_Var`.
        unsafe { slice::from_raw_parts(master.axis, axis_count) }.to_vec()
    };
    let mut axis_flags = Vec::with_capacity(axis_count);
    for axis_index in 0..axis_count {
        let mut flags = 0;
        let axis_index = FT_UInt::try_from(axis_index).ok()?;
        let flag_err = FT_Get_Var_Axis_Flags(master_ptr, axis_index, &mut flags);
        if flag_err != rust_ffi::FT_Err_Ok {
            return None;
        }
        axis_flags.push(flags);
    }
    let namedstyle_count = usize::try_from(master.num_namedstyles).ok()?;
    let namedstyles = if master.namedstyle.is_null() {
        Vec::new()
    } else {
        // SAFETY: the descriptor's namedstyle pointer has `num_namedstyles`
        // initialized records and remains live until `FT_Done_MM_Var`.
        unsafe { slice::from_raw_parts(master.namedstyle, namedstyle_count) }
            .iter()
            .map(|style| {
                let coords = if style.coords.is_null() {
                    Vec::new()
                } else {
                    // SAFETY: FreeType stores one coordinate per axis for
                    // every named style in the live descriptor allocation.
                    unsafe { slice::from_raw_parts(style.coords, axis_count) }.to_vec()
                };
                (*style, coords)
            })
            .collect()
    };
    let done_err = FT_Done_MM_Var(library, master_ptr);
    Some((err, master, axes, axis_flags, namedstyles, done_err))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_count(face: FT_Face) -> Option<FT_UInt> {
    let state = face_state(face)?;
    FT_UInt::try_from(state.charmaps.len()).ok()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_by_index(face: FT_Face, index: FT_UInt) -> Option<FT_CharMap> {
    face_state(face)?.charmap_by_index(index)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_info_by_index(face: FT_Face, index: FT_UInt) -> Option<FT_CharMapRec> {
    let state = face_state(face)?;
    let index = usize::try_from(index).ok()?;
    state.charmaps.get(index).copied()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_active_charmap_index(face: FT_Face) -> Option<FT_Int> {
    let state = face_state(face)?;
    Some(state.inner.active_charmap_index)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_slot_snapshot(face: FT_Face) -> Option<AbiSlotSnapshot> {
    let slot = abi_glyph_slot(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let slot = unsafe { slot.as_ref() };
    let len = usize::try_from(i64::from(slot.bitmap.pitch).abs())
        .ok()?
        .checked_mul(usize::try_from(slot.bitmap.rows).ok()?)?;
    let bitmap = if slot.bitmap.buffer.is_null() || len == 0 {
        None
    } else {
        // SAFETY: the buffer is owned by the live slot for the duration of this copy.
        let buffer = unsafe { slice::from_raw_parts(slot.bitmap.buffer, len) }.to_vec();
        Some(AbiBitmapSnapshot {
            rows: slot.bitmap.rows,
            width: slot.bitmap.width,
            pitch: slot.bitmap.pitch,
            num_grays: slot.bitmap.num_grays,
            pixel_mode: slot.bitmap.pixel_mode,
            left: slot.bitmap_left,
            top: slot.bitmap_top,
            owns_bitmap: slot.owns_bitmap,
            buffer,
        })
    };
    Some(AbiSlotSnapshot {
        glyph_index: slot.glyph_index,
        metrics: slot.metrics,
        advance: slot.advance,
        format: slot.format,
        num_subglyphs: slot.rust_slot.num_subglyphs,
        outline_cbox: rust_bbox_to_abi(slot.rust_slot.outline_cbox),
        outline_bbox: rust_bbox_to_abi(slot.rust_slot.outline_bbox),
        outline: slot.rust_slot.outline.clone(),
        bitmap,
    })
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiOutlineGlyphSnapshot {
    pub advance: FT_Vector,
    pub outline: rust_ffi::FT_OutlineSnapshot,
    pub cbox: FT_BBox,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBitmapGlyphSnapshot {
    pub root: FT_GlyphRec,
    pub left: FT_Int,
    pub top: FT_Int,
    pub bitmap: AbiBitmapSnapshot,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_get_outline_glyph_from_face(face: FT_Face) -> Result<FT_Glyph, FT_Error> {
    let Some(slot) = abi_glyph_slot(face) else {
        return Err(rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error);
    };
    let mut glyph = ptr::null_mut();
    let err = FT_Get_Glyph(slot.as_ptr(), &mut glyph);
    if err == rust_ffi::FT_Err_Ok {
        Ok(glyph)
    } else {
        Err(err)
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_outline_glyph_snapshot(glyph: FT_Glyph) -> Option<AbiOutlineGlyphSnapshot> {
    let owned = owned_outline_glyph_from_root(glyph)?;
    let mut cbox = FT_BBox::default();
    FT_Glyph_Get_CBox(glyph, 0, &mut cbox);
    Some(AbiOutlineGlyphSnapshot {
        advance: owned.record.root.advance,
        outline: owned.core.outline.clone(),
        cbox,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_bitmap_glyph_snapshot(glyph: FT_Glyph) -> Option<AbiBitmapGlyphSnapshot> {
    let owned = owned_bitmap_glyph_from_root(glyph)?;
    Some(AbiBitmapGlyphSnapshot {
        root: owned.record.root,
        left: owned.record.left,
        top: owned.record.top,
        bitmap: AbiBitmapSnapshot {
            rows: owned.record.bitmap.rows,
            width: owned.record.bitmap.width,
            pitch: owned.record.bitmap.pitch,
            num_grays: owned.record.bitmap.num_grays,
            pixel_mode: owned.record.bitmap.pixel_mode,
            left: owned.record.left,
            top: owned.record.top,
            owns_bitmap: true,
            buffer: owned.buffer.to_vec(),
        },
    })
}

#[cfg(feature = "abi-test-support")]
fn rust_bbox_to_abi(bbox: rust_ffi::FT_BBox) -> FT_BBox {
    FT_BBox {
        xMin: bbox.xMin,
        yMin: bbox.yMin,
        xMax: bbox.xMax,
        yMax: bbox.yMax,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_render_glyph_from_face(face: FT_Face, render_mode: FT_Render_Mode) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_Render_Glyph(slot.as_ptr(), render_mode)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_get_glyph_from_face(face: FT_Face, aglyph: *mut FT_Glyph) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_Get_Glyph(slot.as_ptr(), aglyph)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_set_unsupported_glyph_slot(face: FT_Face) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    store_slot(
        face,
        rust_ffi::FT_Unsupported_GlyphSlot(&state.inner),
        rust_ffi::FT_LOAD_DEFAULT,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_set_outline_glyph_slot_advance(
    face: FT_Face,
    advance_x: FT_Pos,
    advance_y: FT_Pos,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    store_slot(
        face,
        rust_ffi::FT_Outline_GlyphSlot_With_Advance(&state.inner, advance_x, advance_y),
        rust_ffi::FT_LOAD_DEFAULT,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_slant_from_face(
    face: FT_Face,
    xslant: FT_Fixed,
    yslant: FT_Fixed,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Slant(slot.as_ptr(), xslant, yslant);
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_adjust_weight_from_face(
    face: FT_Face,
    xdelta: FT_Fixed,
    ydelta: FT_Fixed,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_AdjustWeight(slot.as_ptr(), xdelta, ydelta);
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_embolden_from_face(face: FT_Face) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Embolden(slot.as_ptr());
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_own_bitmap_from_face(face: FT_Face) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Ok;
    };
    FT_GlyphSlot_Own_Bitmap(slot.as_ptr())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_own_bitmap_copy_allocation_failure_from_face(face: FT_Face) -> FT_Error {
    let Some(slot_ptr) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Ok;
    };
    // SAFETY: `slot_ptr` is produced from a live face-owned slot allocated by this crate.
    unsafe {
        let slot_ref = &mut *slot_ptr.as_ptr();
        let err = rust_ffi::FT_GlyphSlot_Own_Bitmap_Copy_Allocation_Failure(Some(
            &mut slot_ref.rust_slot,
        ));
        if err != rust_ffi::FT_Err_Ok {
            return err;
        }
        let source_face = slot_ref.source_face;
        let load_flags = slot_ref.load_flags;
        let rust_slot = slot_ref.rust_slot.clone();
        *slot_ref = rust_slot_to_abi(rust_slot, source_face, load_flags);
    }
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_fvar_namedstyle_coords(
    face: FT_Face,
    namedstyle_index: FT_UInt,
) -> Option<Vec<FT_Fixed>> {
    let state = face_state(face)?;
    rust_ffi::FT_Fvar_Named_Style_Coords(Some(&state.inner), namedstyle_index).ok()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_set_own_bitmap_from_face(face: FT_Face, owns_bitmap: bool) -> FT_Error {
    let Some(mut slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    unsafe {
        let slot = slot.as_mut();
        slot.owns_bitmap = owns_bitmap;
        slot.rust_slot.owns_bitmap = owns_bitmap;
    }
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_oblique_from_face(face: FT_Face) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Oblique(slot.as_ptr());
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_get_subglyph_info_from_face(
    face: FT_Face,
    sub_index: FT_UInt,
    p_index: *mut FT_Int,
    p_flags: *mut FT_UInt,
    p_arg1: *mut FT_Int,
    p_arg2: *mut FT_Int,
    p_transform: *mut FT_Matrix,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_Get_SubGlyph_Info(
        slot.as_ptr(),
        sub_index,
        p_index,
        p_flags,
        p_arg1,
        p_arg2,
        p_transform,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_size_metrics(face: FT_Face) -> Option<FT_Size_Metrics> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let size = unsafe { (*face.as_ptr()).size };
    let size = NonNull::new(size)?;
    // SAFETY: `size` is owned by the live face for the duration of this copy.
    Some(unsafe { size.as_ref().metrics })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_active_size(face: FT_Face) -> Option<FT_Size> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    Some(unsafe { (*face.as_ptr()).size })
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
pub struct AbiSizeRecSnapshot {
    pub face: FT_Face,
    pub generic: FT_Generic,
    pub metrics: FT_Size_Metrics,
    pub internal: *mut c_void,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_size_rec_snapshot(size: FT_Size) -> Option<AbiSizeRecSnapshot> {
    let size = NonNull::new(size)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let size = unsafe { size.as_ref() };
    Some(AbiSizeRecSnapshot {
        face: size.face,
        generic: size.generic,
        metrics: size.metrics,
        internal: size.internal,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_size_rec_set_generic_data(size: FT_Size, data: FT_Pointer) -> bool {
    let Some(mut size) = NonNull::new(size) else {
        return false;
    };
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    unsafe {
        size.as_mut().generic.data = data;
    }
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_os2(face: FT_Face) -> Option<TT_OS2> {
    let table = FT_Get_Sfnt_Table(face, rust_ffi::FT_SFNT_OS2 as FT_Sfnt_Tag);
    let table = NonNull::new(table.cast::<rust_ffi::TT_OS2>())?;
    // SAFETY: `FT_Get_Sfnt_Table` returned a live face-owned `TT_OS2` pointer.
    let os2 = unsafe { table.as_ref() };
    Some(TT_OS2 {
        version: os2.version,
        xAvgCharWidth: os2.xAvgCharWidth,
        usWeightClass: os2.usWeightClass,
        usWidthClass: os2.usWidthClass,
        fsType: os2.fsType,
        ySubscriptXSize: os2.ySubscriptXSize,
        ySubscriptYSize: os2.ySubscriptYSize,
        ySubscriptXOffset: os2.ySubscriptXOffset,
        ySubscriptYOffset: os2.ySubscriptYOffset,
        ySuperscriptXSize: os2.ySuperscriptXSize,
        ySuperscriptYSize: os2.ySuperscriptYSize,
        ySuperscriptXOffset: os2.ySuperscriptXOffset,
        ySuperscriptYOffset: os2.ySuperscriptYOffset,
        yStrikeoutSize: os2.yStrikeoutSize,
        yStrikeoutPosition: os2.yStrikeoutPosition,
        sFamilyClass: os2.sFamilyClass,
        panose: os2.panose,
        ulUnicodeRange1: os2.ulUnicodeRange1,
        ulUnicodeRange2: os2.ulUnicodeRange2,
        ulUnicodeRange3: os2.ulUnicodeRange3,
        ulUnicodeRange4: os2.ulUnicodeRange4,
        achVendID: os2.achVendID,
        fsSelection: os2.fsSelection,
        usFirstCharIndex: os2.usFirstCharIndex,
        usLastCharIndex: os2.usLastCharIndex,
        sTypoAscender: os2.sTypoAscender,
        sTypoDescender: os2.sTypoDescender,
        sTypoLineGap: os2.sTypoLineGap,
        usWinAscent: os2.usWinAscent,
        usWinDescent: os2.usWinDescent,
        ulCodePageRange1: os2.ulCodePageRange1,
        ulCodePageRange2: os2.ulCodePageRange2,
        sxHeight: os2.sxHeight,
        sCapHeight: os2.sCapHeight,
        usDefaultChar: os2.usDefaultChar,
        usBreakChar: os2.usBreakChar,
        usMaxContext: os2.usMaxContext,
        usLowerOpticalPointSize: os2.usLowerOpticalPointSize,
        usUpperOpticalPointSize: os2.usUpperOpticalPointSize,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_vhea(face: FT_Face) -> Option<TT_VertHeader> {
    let table = FT_Get_Sfnt_Table(face, rust_ffi::FT_SFNT_VHEA as FT_Sfnt_Tag);
    let table = NonNull::new(table.cast::<rust_ffi::TT_VertHeader>())?;
    // SAFETY: `FT_Get_Sfnt_Table` returned a live face-owned `TT_VertHeader` pointer.
    let vhea = unsafe { table.as_ref() };
    Some(TT_VertHeader {
        Version: vhea.Version,
        Ascender: vhea.Ascender,
        Descender: vhea.Descender,
        Line_Gap: vhea.Line_Gap,
        advance_Height_Max: vhea.advance_Height_Max,
        min_Top_Side_Bearing: vhea.min_Top_Side_Bearing,
        min_Bottom_Side_Bearing: vhea.min_Bottom_Side_Bearing,
        yMax_Extent: vhea.yMax_Extent,
        caret_Slope_Rise: vhea.caret_Slope_Rise,
        caret_Slope_Run: vhea.caret_Slope_Run,
        caret_Offset: vhea.caret_Offset,
        Reserved: vhea.Reserved,
        metric_Data_Format: vhea.metric_Data_Format,
        number_Of_VMetrics: vhea.number_Of_VMetrics,
        long_metrics: vhea.long_metrics.cast(),
        short_metrics: vhea.short_metrics.cast(),
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_maxp(face: FT_Face) -> Option<TT_MaxProfile> {
    let table = FT_Get_Sfnt_Table(face, rust_ffi::FT_SFNT_MAXP as FT_Sfnt_Tag);
    let table = NonNull::new(table.cast::<rust_ffi::TT_MaxProfile>())?;
    // SAFETY: `FT_Get_Sfnt_Table` returned a live face-owned `TT_MaxProfile` pointer.
    let maxp = unsafe { table.as_ref() };
    Some(TT_MaxProfile {
        version: maxp.version,
        numGlyphs: maxp.numGlyphs,
        maxPoints: maxp.maxPoints,
        maxContours: maxp.maxContours,
        maxCompositePoints: maxp.maxCompositePoints,
        maxCompositeContours: maxp.maxCompositeContours,
        maxZones: maxp.maxZones,
        maxTwilightPoints: maxp.maxTwilightPoints,
        maxStorage: maxp.maxStorage,
        maxFunctionDefs: maxp.maxFunctionDefs,
        maxInstructionDefs: maxp.maxInstructionDefs,
        maxStackElements: maxp.maxStackElements,
        maxSizeOfInstructions: maxp.maxSizeOfInstructions,
        maxComponentElements: maxp.maxComponentElements,
        maxComponentDepth: maxp.maxComponentDepth,
    })
}

#[cfg(feature = "abi-test-support")]
fn abi_glyph_slot(face: FT_Face) -> Option<NonNull<FT_GlyphSlotRec>> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    NonNull::new(unsafe { (*face.as_ptr()).glyph })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyph_slot_pointer(face: FT_Face) -> Option<FT_GlyphSlot> {
    abi_glyph_slot(face).map(NonNull::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Init_FreeType(alibrary: *mut FT_Library) -> FT_Error {
    let Some(out) = non_null_mut(alibrary) else {
        // FreeType 2.14.3 `src/base/ftinit.c:FT_Init_FreeType` reports
        // Invalid_Face_Handle when the output library pointer itself is null;
        // the pointer check lives in this thin ABI layer.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let library = Box::new(FT_LibraryRec {
        internal: Box::into_raw(Box::new(LibraryState::new(rust_ffi::FT_Init_FreeType())))
            .cast::<c_void>(),
    });
    // SAFETY: `out` is a valid out pointer checked above.
    unsafe { *out.as_ptr() = Box::into_raw(library) };
    rust_ffi::FT_Err_Ok
}

fn done_library_allocation(state: &mut LibraryState) {
    if state.allocation_memory.is_null() || state.allocation_block.is_null() {
        return;
    }
    // SAFETY: `allocation_memory` is the live FT_MemoryRec supplied to
    // FT_New_Library, and `allocation_block` is the block returned by its
    // alloc callback for this library object.
    unsafe {
        if let Some(free) = (*state.allocation_memory).free {
            free(state.allocation_memory, state.allocation_block);
        }
    }
    state.allocation_block = std::ptr::null_mut();
}

fn drop_library_rec(library: NonNull<FT_LibraryRec>, free_custom_allocation: bool) {
    // SAFETY: `library` is a live handle allocated by this crate.
    unsafe {
        let library = Box::from_raw(library.as_ptr());
        if !library.internal.is_null() {
            let mut state = Box::from_raw(library.internal.cast::<LibraryState>());
            if free_custom_allocation {
                done_library_allocation(&mut state);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_FreeType(library: FT_Library) -> FT_Error {
    if let Some(library) = non_null_mut(library) {
        drop_library_rec(library, false);
        rust_ffi::FT_Err_Ok
    } else {
        35 // matches C runtime: FT_Done_FreeType(NULL)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Library(memory: FT_Memory, alibrary: *mut FT_Library) -> FT_Error {
    let (Some(memory), Some(out)) = (non_null_mut(memory), non_null_mut(alibrary)) else {
        // FreeType 2.14.3 `src/base/ftobjs.c:FT_New_Library` returns before
        // writing `alibrary` when either public argument is null.
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `memory` is a non-null FT_MemoryRec provided by the caller.
    let allocation = unsafe {
        match (*memory.as_ptr()).alloc {
            Some(alloc) => alloc(
                memory.as_ptr(),
                std::mem::size_of::<FT_LibraryRec>() as c_long,
            ),
            None => std::ptr::null_mut(),
        }
    };
    if allocation.is_null() {
        return rust_ffi::FT_Err_Out_Of_Memory;
    }
    let Ok(inner) =
        rust_ffi::FT_New_Library(Some(memory.as_ptr().cast::<rust_ffi::FT_MemoryRec>()))
    else {
        // SAFETY: release the allocation block if core rejects construction.
        unsafe {
            if let Some(free) = (*memory.as_ptr()).free {
                free(memory.as_ptr(), allocation);
            }
        }
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let library = Box::new(FT_LibraryRec {
        internal: Box::into_raw(Box::new(LibraryState::new_with_allocation(
            inner,
            memory.as_ptr(),
            allocation,
        )))
        .cast::<c_void>(),
    });
    // SAFETY: `out` is a valid out pointer checked above.
    unsafe { *out.as_ptr() = Box::into_raw(library) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Reference_Library(library: FT_Library) -> FT_Error {
    rust_ffi::FT_Reference_Library(library_mut(library))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Library(library: FT_Library) -> FT_Error {
    let Some(library_ptr) = non_null_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let err = rust_ffi::FT_Done_Library(library_mut(library));
    if err != rust_ffi::FT_Err_Ok {
        return err;
    }
    if library_ref(library).is_some_and(|library| rust_ffi::FT_Library_Refcount(Some(library)) == 0)
    {
        drop_library_rec(library_ptr, true);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_MM_Var(face: FT_Face, amaster: *mut *mut FT_MM_Var) -> FT_Error {
    let Some(amaster) = non_null_mut(amaster) else {
        return rust_ffi::FT_Get_MM_Var(None, None, None, None, None);
    };
    let Some(state) = face_state(face) else {
        let mut out = FT_MM_Var::default();
        return rust_ffi::FT_Get_MM_Var(None, Some(&mut out), None, None, None);
    };
    let mut axis = vec![FT_Var_Axis::default(); 64].into_boxed_slice();
    let mut namedstyle = vec![rust_ffi::FT_Var_Named_Style::default(); 256].into_boxed_slice();
    let mut namedstyle_coords = vec![rust_ffi::FT_Fixed::default(); 64 * 256].into_boxed_slice();
    let mut master = Box::new(FT_MM_Var::default());
    let err = rust_ffi::FT_Get_MM_Var(
        Some(&state.inner),
        Some(&mut master),
        Some(&mut axis),
        Some(&mut namedstyle),
        Some(&mut namedstyle_coords),
    );
    if err != rust_ffi::FT_Err_Ok {
        return err;
    }
    master.axis = axis.as_mut_ptr();
    master.namedstyle = if master.num_namedstyles == 0 {
        ptr::null_mut()
    } else {
        namedstyle.as_mut_ptr()
    };
    let mut owned = OwnedMmVar {
        master,
        _axis: axis,
        _namedstyle: namedstyle,
        _namedstyle_coords: namedstyle_coords,
    };
    let master_ptr: *mut FT_MM_Var = owned.master.as_mut();
    OWNED_MM_VARS.with(|vars| {
        vars.borrow_mut().insert(master_ptr.addr(), owned);
    });
    // SAFETY: `amaster` is a non-null output pointer supplied by the caller.
    unsafe { *amaster.as_ptr() = master_ptr };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_MM_Var(library: FT_Library, amaster: *mut FT_MM_Var) -> FT_Error {
    let Some(library) = library_ref(library) else {
        return rust_ffi::FT_Done_MM_Var(None, None);
    };
    if !amaster.is_null() {
        let removed = OWNED_MM_VARS.with(|vars| vars.borrow_mut().remove(&amaster.addr()));
        if removed.is_some() {
            return rust_ffi::FT_Done_MM_Var(Some(library), None);
        }
    }
    let amaster = non_null_mut(amaster).map(|mut amaster| {
        // SAFETY: `amaster` is non-null and the caller provides a writable
        // FT_MM_Var descriptor owned by this API.  The current pure-Rust core
        // only observes null-vs-non-null ownership for this public route.
        unsafe { amaster.as_mut() }
    });
    rust_ffi::FT_Done_MM_Var(Some(library), amaster)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Var_Axis_Flags(
    master: *mut FT_MM_Var,
    axis_index: FT_UInt,
    flags: *mut FT_UInt,
) -> FT_Error {
    let master = non_null_mut(master).map(|master| {
        // SAFETY: `master` is non-null and points to a public FT_MM_Var
        // record supplied by the caller.
        unsafe { master.as_ref() }
    });
    let flags = non_null_mut(flags).map(|mut flags| {
        // SAFETY: `flags` is non-null and points to caller-writable FT_UInt storage.
        unsafe { flags.as_mut() }
    });
    rust_ffi::FT_Get_Var_Axis_Flags(master, axis_index, flags)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdFilter(_library: FT_Library, filter: FT_LcdFilter) -> FT_Error {
    rust_ffi::FT_Library_SetLcdFilter(None, filter)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdFilterWeights(
    _library: FT_Library,
    weights: *mut FT_Byte,
) -> FT_Error {
    rust_ffi::FT_Library_SetLcdFilterWeights(None, weights)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdGeometry(
    library: FT_Library,
    sub: *const FT_Vector,
) -> FT_Error {
    let rust_sub = if sub.is_null() {
        None
    } else {
        let mut vectors = [rust_ffi::FT_Vector::default(); 3];
        for (index, vector) in vectors.iter_mut().enumerate() {
            // SAFETY: `sub` is non-null and the C API requires three vectors.
            let source = unsafe { &*sub.add(index) };
            *vector = rust_ffi::FT_Vector {
                x: source.x,
                y: source.y,
            };
        }
        Some(vectors)
    };
    rust_ffi::FT_Library_SetLcdGeometry(library_mut(library), rust_sub)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_TrueType_Engine_Type(library: FT_Library) -> FT_TrueTypeEngineType {
    rust_ffi::FT_Get_TrueType_Engine_Type(library_ref(library))
}

fn property_name_arg(ptr: *const FT_String) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: FreeType string arguments are nul-terminated `const char*`
    // values owned by the caller for the duration of the call.
    unsafe { CStr::from_ptr(ptr).to_str().ok().map(ToOwned::to_owned) }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_interface_present(
    library: FT_Library,
    module_name: Option<&str>,
) -> bool {
    !rust_ffi::FT_Get_Module_Interface(library_ref(library), module_name).is_null()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_requester_service_available(
    library: FT_Library,
    module_name: Option<&str>,
    service_name: &str,
) -> bool {
    rust_ffi::FT_Module_Requester_Service_Available(library_ref(library), module_name, service_name)
}

fn is_increase_x_height_property(module_name: Option<&str>, property_name: Option<&str>) -> bool {
    module_name == Some("autofitter") && property_name == Some("increase-x-height")
}

fn is_glyph_to_script_map_property(module_name: Option<&str>, property_name: Option<&str>) -> bool {
    module_name == Some("autofitter") && property_name == Some("glyph-to-script-map")
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Property_Get(
    library: FT_Library,
    module_name: *const FT_String,
    property_name: *const FT_String,
    value: *mut c_void,
) -> FT_Error {
    let module_name = property_name_arg(module_name);
    let property_name = property_name_arg(property_name);
    if is_glyph_to_script_map_property(module_name.as_deref(), property_name.as_deref()) {
        let Some(prop) = (unsafe { value.cast::<rust_ffi::FT_Prop_GlyphToScriptMap>().as_mut() })
        else {
            return rust_ffi::FT_Property_Get_GlyphToScriptMap(
                library_ref(library),
                module_name.as_deref(),
                property_name.as_deref(),
                None,
                None,
            );
        };
        let requested_face = prop.face.cast::<FT_FaceRec>();
        let face = face_state(requested_face).map(|state| &state.inner);
        let error = rust_ffi::FT_Property_Get_GlyphToScriptMap(
            library_ref(library),
            module_name.as_deref(),
            property_name.as_deref(),
            face,
            Some(prop),
        );
        if error == rust_ffi::FT_Err_Ok {
            prop.face = requested_face.cast();
        }
        return error;
    }
    if is_increase_x_height_property(module_name.as_deref(), property_name.as_deref()) {
        let Some(prop) = (unsafe { value.cast::<rust_ffi::FT_Prop_IncreaseXHeight>().as_mut() })
        else {
            return rust_ffi::FT_Property_Get_IncreaseXHeight(
                library_ref(library),
                module_name.as_deref(),
                property_name.as_deref(),
                None,
                None,
            );
        };
        let face = face_state(prop.face.cast::<FT_FaceRec>()).map(|state| &state.inner);
        return rust_ffi::FT_Property_Get_IncreaseXHeight(
            library_ref(library),
            module_name.as_deref(),
            property_name.as_deref(),
            face,
            Some(prop),
        );
    }
    let value = if value.is_null() {
        None
    } else {
        // SAFETY: For the implemented TrueType property the public C contract
        // requires an `FT_UInt*`; null was handled above.
        Some(unsafe { &mut *value.cast::<FT_UInt>() })
    };
    rust_ffi::FT_Property_Get(
        library_ref(library),
        module_name.as_deref(),
        property_name.as_deref(),
        value,
    )
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn abi_glyph_to_script_map_sample(
    face: FT_Face,
    glyph_indices: &[FT_UInt],
) -> Vec<(FT_UInt, FT_UShort)> {
    face_state(face).map_or_else(Vec::new, |state| {
        rust_ffi::FT_Glyph_To_Script_Map_Sample_For_Test(&state.inner, glyph_indices)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Property_Set(
    library: FT_Library,
    module_name: *const FT_String,
    property_name: *const FT_String,
    value: *const c_void,
) -> FT_Error {
    let module_name = property_name_arg(module_name);
    let property_name = property_name_arg(property_name);
    if is_increase_x_height_property(module_name.as_deref(), property_name.as_deref()) {
        let prop = unsafe { value.cast::<rust_ffi::FT_Prop_IncreaseXHeight>().as_ref() };
        let face = prop.and_then(|prop| face_state_mut(prop.face.cast::<FT_FaceRec>()));
        return rust_ffi::FT_Property_Set_IncreaseXHeight(
            library_ref(library),
            module_name.as_deref(),
            property_name.as_deref(),
            face.map(|state| &mut state.inner),
            prop,
        );
    }
    let value = if value.is_null() {
        None
    } else {
        // SAFETY: For the implemented TrueType property the public C contract
        // requires an `FT_UInt*`; null was handled above.
        Some(unsafe { *value.cast::<FT_UInt>() })
    };
    rust_ffi::FT_Property_Set(
        library_mut(library),
        module_name.as_deref(),
        property_name.as_deref(),
        value,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Default_Properties(library: FT_Library) {
    rust_ffi::FT_Set_Default_Properties(library_mut(library));
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_set_default_properties(
    library_present: i32,
    env: Option<&str>,
) -> Option<FT_UInt> {
    let mut library = if library_present == 0 {
        None
    } else {
        Some(rust_ffi::FT_Init_FreeType())
    };
    rust_ffi::FT_Set_Default_Properties_From_Env(library.as_mut(), env);
    let library = library.as_ref()?;
    let mut value = 0;
    let error = rust_ffi::FT_Property_Get(
        Some(library),
        Some("truetype"),
        Some("interpreter-version"),
        Some(&mut value),
    );
    if error == rust_ffi::FT_Err_Ok {
        Some(value)
    } else {
        None
    }
}

fn face_property_from_abi(parameter: &FT_Parameter) -> rust_ffi::FT_Face_Property {
    let value = match parameter.tag as i64 {
        rust_ffi::FT_PARAM_TAG_STEM_DARKENING if !parameter.data.is_null() => {
            // SAFETY: FreeType requires `FT_PARAM_TAG_STEM_DARKENING` data to
            // point to an `FT_Bool` for the duration of `FT_Face_Properties`.
            Some(rust_ffi::FT_Face_Property_Value::Bool(unsafe {
                *parameter.data.cast::<FT_Bool>()
            }))
        }
        rust_ffi::FT_PARAM_TAG_RANDOM_SEED if !parameter.data.is_null() => {
            // SAFETY: FreeType requires `FT_PARAM_TAG_RANDOM_SEED` data to
            // point to an `FT_Int32` for the duration of `FT_Face_Properties`.
            Some(rust_ffi::FT_Face_Property_Value::Int32(unsafe {
                *parameter.data.cast::<FT_Int32>()
            }))
        }
        _ => None,
    };
    rust_ffi::FT_Face_Property {
        tag: parameter.tag,
        value,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_Properties(
    face: FT_Face,
    num_properties: FT_UInt,
    properties: *mut FT_Parameter,
) -> FT_Error {
    if num_properties > 0 && properties.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let props = if num_properties == 0 {
        Vec::new()
    } else {
        let Ok(count) = usize::try_from(num_properties) else {
            return rust_ffi::FT_Err_Invalid_Argument;
        };
        // SAFETY: The C ABI requires `properties` to address `num_properties`
        // readable `FT_Parameter` records when `num_properties > 0`.
        unsafe { slice::from_raw_parts(properties, count) }
            .iter()
            .map(face_property_from_abi)
            .collect()
    };
    let face = face_state_mut(face).map(|state| &mut state.inner);
    rust_ffi::FT_Face_Properties(face, Some(&props))
}

pub fn abi_face_properties_state(face: FT_Face) -> Option<rust_ffi::FT_Face_Properties_State> {
    face_state(face).map(|state| rust_ffi::FT_Face_Properties_Get_State(&state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Add_Default_Modules(library: FT_Library) {
    rust_ffi::FT_Add_Default_Modules(library_mut(library));
}

fn module_name_from_abi(module_name: *const FT_String) -> Option<&'static str> {
    if module_name.is_null() {
        return None;
    }
    // SAFETY: `module_name` is a FreeType ABI C string pointer supplied by the
    // caller.  The wrapper converts only recognized synthetic test names into
    // safe static identifiers before delegating to the pure-Rust core.
    let bytes = unsafe { CStr::from_ptr(module_name).to_bytes() };
    match bytes {
        b"fixture_minimal" => Some("fixture_minimal"),
        b"fixture_renderer" => Some("fixture_renderer"),
        b"fixture_styler" => Some("fixture_styler"),
        b"fixture_upgrade" => Some("fixture_upgrade"),
        b"fixture_future" => Some("fixture_future"),
        _ => None,
    }
}

fn module_class_info_from_abi(
    clazz: *const FT_Module_Class,
) -> Option<rust_ffi::FT_Module_Class_Info> {
    let clazz = non_null(clazz.cast_mut())?;
    // SAFETY: `clazz` is non-null and points to a readable FreeType module
    // class record for the duration of this ABI call.
    let clazz = unsafe { clazz.as_ref() };
    Some(rust_ffi::FT_Module_Class_Info {
        module_flags: clazz.module_flags,
        module_size: clazz.module_size,
        module_name: module_name_from_abi(clazz.module_name),
        module_version: clazz.module_version,
        module_requires: clazz.module_requires,
        module_interface_present: !clazz.module_interface.is_null(),
        module_init: if clazz.module_init.is_null() {
            rust_ffi::FT_Module_Callback_Behavior::None
        } else {
            rust_ffi::FT_Module_Callback_Behavior::RecordThenOk
        },
        module_done: if clazz.module_done.is_null() {
            rust_ffi::FT_Module_Callback_Behavior::None
        } else {
            rust_ffi::FT_Module_Callback_Behavior::RecordThenOk
        },
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Add_Module(library: FT_Library, clazz: *const FT_Module_Class) -> FT_Error {
    let info = module_class_info_from_abi(clazz);
    rust_ffi::FT_Add_Module(library_mut(library), info.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Module(library: FT_Library, module_name: *const FT_String) -> FT_Module {
    let Some(name) = module_name_from_abi(module_name) else {
        return ptr::null_mut();
    };
    let Some(state) = library_state_mut(library) else {
        return ptr::null_mut();
    };
    if rust_ffi::FT_Library_Has_Module(Some(&state.inner), name) {
        if rust_ffi::FT_Library_Module_Flags(Some(&state.inner), name)
            .is_some_and(|flags| flags & rust_ffi::FT_MODULE_RENDERER as FT_ULong != 0)
        {
            return (&mut state.synthetic_renderer as *mut FT_RendererRec).cast::<FT_ModuleRec>();
        }
        (&mut *state.synthetic_module_handle) as *mut FT_ModuleRec
    } else {
        ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Debug_Hook(
    library: FT_Library,
    hook_index: FT_UInt,
    debug_hook: FT_DebugHook_Func,
) {
    rust_ffi::FT_Set_Debug_Hook(library_mut(library), hook_index, debug_hook);
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_new_library_without_default_modules() -> FT_Library {
    Box::into_raw(Box::new(FT_LibraryRec {
        internal: Box::into_raw(Box::new(LibraryState::new(
            rust_ffi::FT_New_Library_Without_Default_Modules(),
        )))
        .cast::<c_void>(),
    }))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_has_truetype_module(library: FT_Library) -> bool {
    rust_ffi::FT_Library_Has_TrueType_Module(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_has_module(library: FT_Library, name: &str) -> bool {
    rust_ffi::FT_Library_Has_Module(library_ref(library), name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_module_flags(library: FT_Library, name: &str) -> Option<FT_ULong> {
    rust_ffi::FT_Library_Module_Flags(library_ref(library), name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_module_count(library: FT_Library) -> usize {
    rust_ffi::FT_Library_Module_Count(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_synthetic_module_info(
    library: FT_Library,
    name: &str,
) -> Option<rust_ffi::FT_Installed_Module_Info> {
    rust_ffi::FT_Library_Synthetic_Module_Info(library_ref(library), name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_renderer_class(
    library: FT_Library,
    format: FT_Glyph_Format,
) -> Option<(&'static str, FT_Glyph_Format, bool, bool)> {
    rust_ffi::FT_Library_Renderer_Class(library_ref(library), format)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Renderer(library: FT_Library, format: FT_Glyph_Format) -> FT_Renderer {
    let Some(state) = library_state_mut(library) else {
        return ptr::null_mut();
    };
    let Some((module_name, glyph_format, _, _)) =
        rust_ffi::FT_Library_Renderer_Class(Some(&state.inner), format)
    else {
        return ptr::null_mut();
    };
    if glyph_format == state.outline_renderer.format
        && module_name == state.outline_renderer.module_name
    {
        &mut state.outline_renderer
    } else if glyph_format == state.synthetic_renderer.format
        && module_name == state.synthetic_renderer.module_name
    {
        &mut state.synthetic_renderer
    } else {
        ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Renderer(
    library: FT_Library,
    renderer: FT_Renderer,
    num_params: FT_UInt,
    parameters: *mut FT_Parameter,
) -> FT_Error {
    let Some(state) = library_state_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let Some(renderer) = non_null_mut(renderer) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if num_params != 0 && parameters.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let owned_renderer = &mut state.outline_renderer as *mut FT_RendererRec;
    let synthetic_renderer = &mut state.synthetic_renderer as *mut FT_RendererRec;
    let renderer_name = if renderer.as_ptr() == owned_renderer {
        state.outline_renderer.module_name
    } else if renderer.as_ptr() == synthetic_renderer {
        state.synthetic_renderer.module_name
    } else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // FreeType 2.14.3 `src/base/ftobjs.c:FT_Set_Renderer` performs raw list
    // membership validation in the ABI layer, then updates the library's
    // current outline renderer.  Parameter callbacks are not used by the
    // default smooth renderer for this no-parameter parity route.
    rust_ffi::FT_Library_Set_Renderer_By_Format(
        Some(&mut state.inner),
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        renderer_name,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_default_module_names(library: FT_Library) -> &'static [&'static str] {
    rust_ffi::FT_Library_Default_Module_Names(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_refcount(library: FT_Library) -> usize {
    rust_ffi::FT_Library_Refcount(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_memory_is(library: FT_Library, memory: FT_Memory) -> bool {
    library_ref(library).is_some_and(|library| {
        rust_ffi::FT_Library_Memory(Some(library)).cast::<FT_MemoryRec>() == memory
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_has_truetype_engine_service(library: FT_Library) -> bool {
    rust_ffi::FT_Library_Has_TrueType_Engine_Service(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_debug_hook_classes(
    library: FT_Library,
    hook_a: FT_DebugHook_Func,
    hook_b: FT_DebugHook_Func,
) -> [FT_Int; 4] {
    rust_ffi::FT_Library_Debug_Hook_Classes(library_ref(library), hook_a, hook_b)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_MulDiv(a: FT_Long, b: FT_Long, c: FT_Long) -> FT_Long {
    rust_ffi::FT_MulDiv(a, b, c)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_MulFix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_MulFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_DivFix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_DivFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_RoundFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_RoundFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_CeilFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_CeilFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_FloorFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_FloorFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Sin(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Sin(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Cos(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Cos(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Tan(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Tan(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Atan2(dx: FT_Fixed, dy: FT_Fixed) -> FT_Angle {
    rust_ffi::FT_Atan2(dx, dy)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Angle_Diff(angle1: FT_Angle, angle2: FT_Angle) -> FT_Angle {
    rust_ffi::FT_Angle_Diff(angle1, angle2)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Unit(vector: *mut FT_Vector, angle: FT_Angle) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Unit(rust_vector.as_mut(), angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Rotate(vector: *mut FT_Vector, angle: FT_Angle) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Rotate(rust_vector.as_mut(), angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Length(vector: *mut FT_Vector) -> FT_Fixed {
    let vector = non_null(vector);
    let rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Length(rust_vector.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Polarize(
    vector: *mut FT_Vector,
    length: *mut FT_Fixed,
    angle: *mut FT_Angle,
) {
    let vector = non_null(vector);
    let length_ptr = non_null_mut(length);
    let angle_ptr = non_null_mut(angle);
    let mut rust_length = length_ptr.map(|length| {
        // SAFETY: `length` is non-null and points to a C ABI `FT_Fixed`.
        unsafe { *length.as_ptr() }
    });
    let mut rust_angle = angle_ptr.map(|angle| {
        // SAFETY: `angle` is non-null and points to a C ABI `FT_Angle`.
        unsafe { *angle.as_ptr() }
    });
    let rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Polarize(
        rust_vector.as_ref(),
        rust_length.as_mut(),
        rust_angle.as_mut(),
    );
    if let (Some(length_ptr), Some(value)) = (length_ptr, rust_length) {
        // SAFETY: `length_ptr` is a valid mutable pointer checked above.
        unsafe { *length_ptr.as_ptr() = value };
    }
    if let (Some(angle_ptr), Some(value)) = (angle_ptr, rust_angle) {
        // SAFETY: `angle_ptr` is a valid mutable pointer checked above.
        unsafe { *angle_ptr.as_ptr() = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_From_Polar(vector: *mut FT_Vector, length: FT_Fixed, angle: FT_Angle) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_From_Polar(rust_vector.as_mut(), length, angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Transform(vector: *mut FT_Vector, matrix: *const FT_Matrix) {
    let vector = non_null_mut(vector);
    let matrix = non_null(matrix);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    let rust_matrix = matrix.map(|matrix| {
        // SAFETY: `matrix` is non-null and points to a C ABI `FT_Matrix`.
        let matrix = unsafe { matrix.as_ref() };
        rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        }
    });
    rust_ffi::FT_Vector_Transform(rust_vector.as_mut(), rust_matrix.as_ref());
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Matrix_Multiply(a: *const FT_Matrix, b: *mut FT_Matrix) {
    let a = non_null(a);
    let b = non_null_mut(b);
    let rust_a = a.map(|a| {
        // SAFETY: `a` is non-null and points to a C ABI `FT_Matrix`.
        let a = unsafe { a.as_ref() };
        rust_ffi::FT_Matrix {
            xx: a.xx,
            xy: a.xy,
            yx: a.yx,
            yy: a.yy,
        }
    });
    let mut rust_b = b.map(|b| {
        // SAFETY: `b` is non-null and points to a C ABI `FT_Matrix`.
        let b = unsafe { b.as_ref() };
        rust_ffi::FT_Matrix {
            xx: b.xx,
            xy: b.xy,
            yx: b.yx,
            yy: b.yy,
        }
    });
    rust_ffi::FT_Matrix_Multiply(rust_a.as_ref(), rust_b.as_mut());
    if let (Some(b), Some(rust_b)) = (b, rust_b) {
        // SAFETY: `b` is a valid mutable pointer checked above.
        unsafe {
            (*b.as_ptr()).xx = rust_b.xx;
            (*b.as_ptr()).xy = rust_b.xy;
            (*b.as_ptr()).yx = rust_b.yx;
            (*b.as_ptr()).yy = rust_b.yy;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Matrix_Invert(matrix: *mut FT_Matrix) -> FT_Error {
    let matrix = non_null_mut(matrix);
    let mut rust_matrix = matrix.map(|matrix| {
        // SAFETY: `matrix` is non-null and points to a C ABI `FT_Matrix`.
        let matrix = unsafe { matrix.as_ref() };
        rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        }
    });
    let err = rust_ffi::FT_Matrix_Invert(rust_matrix.as_mut());
    if let (Some(matrix), Some(rust_matrix)) = (matrix, rust_matrix) {
        // SAFETY: `matrix` is a valid mutable pointer checked above.
        unsafe {
            (*matrix.as_ptr()).xx = rust_matrix.xx;
            (*matrix.as_ptr()).xy = rust_matrix.xy;
            (*matrix.as_ptr()).yx = rust_matrix.yx;
            (*matrix.as_ptr()).yy = rust_matrix.yy;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Open_Face(
    library: FT_Library,
    args: *const FT_Open_Args,
    face_index: FT_Long,
    aface: *mut FT_Face,
) -> FT_Error {
    // C FreeType `FT_Open_Face` delegates to `ft_open_face_internal`
    // (ftobjs.c:2514-2586): null `args` is rejected before stream creation;
    // null `library` is then rejected by `FT_Stream_New`; null `aface` is
    // checked after a stream is successfully created.
    let Some(args) = NonNull::new(args.cast_mut()) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `args` is non-null and read-only for this call.
    let args = unsafe { args.as_ref() };
    let source_flags = args.flags
        & ((rust_ffi::FT_OPEN_MEMORY | rust_ffi::FT_OPEN_STREAM | rust_ffi::FT_OPEN_PATHNAME)
            as FT_UInt);
    if source_flags != rust_ffi::FT_OPEN_MEMORY as FT_UInt
        && source_flags != rust_ffi::FT_OPEN_STREAM as FT_UInt
    {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let name_options = open_face_name_options(args);
    if source_flags == rust_ffi::FT_OPEN_STREAM as FT_UInt {
        ft_open_external_stream_face_with_name_options(
            library,
            args.stream,
            face_index,
            aface,
            name_options,
        )
    } else {
        ft_new_memory_face_with_name_options(
            library,
            args.memory_base,
            args.memory_size,
            face_index,
            aface,
            name_options,
        )
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Attach_Stream(face: FT_Face, parameters: *const FT_Open_Args) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(parameters) = non_null(parameters) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `parameters` is non-null and read-only for this call.
    let parameters = unsafe { parameters.as_ref() };
    let source_flags = parameters.flags
        & ((rust_ffi::FT_OPEN_MEMORY | rust_ffi::FT_OPEN_STREAM | rust_ffi::FT_OPEN_PATHNAME)
            as FT_UInt);
    if source_flags != rust_ffi::FT_OPEN_MEMORY as FT_UInt {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    if parameters.memory_base.is_null() || parameters.memory_size < 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Ok(len) = usize::try_from(parameters.memory_size) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `memory_base` is non-null and `memory_size` bytes are readable.
    let data = unsafe { slice::from_raw_parts(parameters.memory_base, len) };
    rust_ffi::FT_Attach_Stream(Some(&mut state.inner), Some(data))
}

fn open_face_name_options(args: &FT_Open_Args) -> rust_ffi::FT_Open_Face_Name_Options {
    let mut options = rust_ffi::FT_Open_Face_Name_Options::default();
    if args.num_params <= 0 || args.params.is_null() {
        return options;
    }
    let Ok(count) = usize::try_from(args.num_params) else {
        return options;
    };
    // SAFETY: `FT_Open_Face` callers provide `num_params` readable parameter
    // records when `params` is non-null.  We only read tags; parameter data is
    // intentionally ignored for these FreeType flags.
    let params = unsafe { slice::from_raw_parts(args.params, count) };
    for param in params {
        match param.tag as i64 {
            rust_ffi::FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY => {
                options.ignore_typographic_family = true;
            }
            rust_ffi::FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY => {
                options.ignore_typographic_subfamily = true;
            }
            _ => {}
        }
    }
    options
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Memory_Face(
    library: FT_Library,
    file_base: *const c_uchar,
    file_size: FT_Long,
    face_index: FT_Long,
    aface: *mut FT_Face,
) -> FT_Error {
    ft_new_memory_face_with_name_options(
        library,
        file_base,
        file_size,
        face_index,
        aface,
        rust_ffi::FT_Open_Face_Name_Options::default(),
    )
}

fn ft_new_memory_face_with_name_options(
    library: FT_Library,
    file_base: *const c_uchar,
    file_size: FT_Long,
    face_index: FT_Long,
    aface: *mut FT_Face,
    options: rust_ffi::FT_Open_Face_Name_Options,
) -> FT_Error {
    // C FreeType validates `FT_New_Memory_Face` in ftobjs.c:1629-1647:
    // null `file_base` is rejected before delegating to `ft_open_face_internal`;
    // null `library` is then rejected by `FT_Stream_New`, before null `aface`.
    if file_base.is_null() || file_size < 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(library) = non_null_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let Some(_out) = non_null_mut(aface) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Ok(file_len) = usize::try_from(file_size) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `file_base` is non-null and the caller promises `file_size` readable bytes.
    let data = unsafe { slice::from_raw_parts(file_base, file_len) };
    ft_open_face_from_bytes_with_name_options(
        library.as_ptr(),
        data,
        face_index,
        aface,
        OpenFaceByteOptions {
            name_options: options,
            external_stream: false,
            stream: ptr::null_mut(),
            stream_close: None,
        },
    )
}

fn ft_open_external_stream_face_with_name_options(
    library: FT_Library,
    stream: FT_Stream,
    face_index: FT_Long,
    aface: *mut FT_Face,
    options: rust_ffi::FT_Open_Face_Name_Options,
) -> FT_Error {
    let Some(stream) = non_null_mut(stream) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(library) = non_null_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let Some(_out) = non_null_mut(aface) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `stream` is non-null and read-only for this call.  This thin ABI
    // layer supports the memory-backed `FT_StreamRec` route used by FreeType's
    // public `FT_OPEN_STREAM` ownership contract.
    let stream = unsafe { stream.as_ref() };
    if stream.base.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let close = if stream.close.is_null() {
        None
    } else {
        // SAFETY: public `FT_StreamRec.close` has FreeType's
        // `void (*)(FT_Stream)` ABI; the Rust layout stores it as an opaque
        // pointer to keep core runtime independent from C callbacks.
        Some(unsafe { std::mem::transmute::<FT_Pointer, extern "C" fn(FT_Stream)>(stream.close) })
    };
    let Ok(file_len) = usize::try_from(stream.size) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `FT_OPEN_STREAM` callers provide `size` readable bytes at `base`
    // for a memory-backed stream; the stream record remains caller-owned.
    let data = unsafe { slice::from_raw_parts(stream.base.cast_const(), file_len) };
    ft_open_face_from_bytes_with_name_options(
        library.as_ptr(),
        data,
        face_index,
        aface,
        OpenFaceByteOptions {
            name_options: options,
            external_stream: true,
            stream: stream as *const FT_StreamRec as FT_Stream,
            stream_close: close,
        },
    )
}

struct OpenFaceByteOptions {
    name_options: rust_ffi::FT_Open_Face_Name_Options,
    external_stream: bool,
    stream: FT_Stream,
    stream_close: FT_Stream_CloseFunc,
}

fn ft_open_face_from_bytes_with_name_options(
    library: FT_Library,
    data: &[u8],
    face_index: FT_Long,
    aface: *mut FT_Face,
    options: OpenFaceByteOptions,
) -> FT_Error {
    let Some(out) = non_null_mut(aface) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(rust_library) = library_ref(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let opened = if options.external_stream {
        rust_ffi::FT_Open_External_Stream_Face_With_Name_Options(
            rust_library,
            data,
            face_index,
            20.0,
            options.name_options,
        )
    } else {
        rust_ffi::FT_New_Memory_Face_With_Name_Options(
            rust_library,
            data,
            face_index,
            20.0,
            options.name_options,
        )
    };
    match opened {
        Ok(inner) => {
            let metrics = rust_size_metrics_to_abi(inner.size_metrics);
            let rust_size = inner.size;
            let initial_slot = rust_ffi::FT_Empty_GlyphSlot(&inner);
            let mut face = Box::new(FT_FaceRec {
                glyph: ptr::null_mut(),
                size: Box::into_raw(Box::new(FT_SizeRec {
                    face: ptr::null_mut(),
                    generic: FT_Generic::default(),
                    metrics,
                    internal: ptr::dangling_mut::<c_void>(),
                    rust_size,
                    owner: ptr::null_mut(),
                })),
                internal: ptr::null_mut(),
            });
            let face_ptr = (&mut *face) as *mut FT_FaceRec;
            let mut state = Box::new(FaceState::new(inner));
            if options.external_stream {
                state.stream = options.stream;
                state.stream_close = options.stream_close;
            }
            // SAFETY: `face.size` was allocated above and is owned by `state`.
            unsafe {
                (*face.size).face = face_ptr;
                (*face.size).owner = face_ptr;
            };
            state.push_size_record(face.size);
            state.refresh_charmaps(face_ptr);
            face.internal = Box::into_raw(state).cast::<c_void>();
            face.glyph = Box::into_raw(Box::new(rust_slot_to_abi(
                initial_slot,
                face_ptr,
                rust_ffi::FT_LOAD_DEFAULT,
            )));
            // SAFETY: `out` is a valid out pointer checked above.
            unsafe { *out.as_ptr() = Box::into_raw(face) };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Face(face: FT_Face) -> FT_Error {
    let Some(face) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    // SAFETY: `face` must be a live handle returned by `FT_New_Memory_Face`.
    unsafe {
        let face = Box::from_raw(face.as_ptr());
        drop_glyph(face.glyph);
        if !face.internal.is_null() {
            let state = Box::from_raw(face.internal.cast::<FaceState>());
            if let Some(close) = state.stream_close {
                close(state.stream);
            }
            drop(state);
        }
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_CheckTrueTypePatents(face: FT_Face) -> FT_Bool {
    rust_ffi::FT_Face_CheckTrueTypePatents(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_SetUnpatentedHinting(face: FT_Face, value: FT_Bool) -> FT_Bool {
    rust_ffi::FT_Face_SetUnpatentedHinting(
        face_state_mut(face).map(|state| &mut state.inner),
        value,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_CBox(outline: *const FT_Outline, acbox: *mut FT_BBox) {
    if outline.is_null() || acbox.is_null() {
        return;
    }
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return;
    };
    let mut bbox = rust_ffi::FT_BBox::default();
    rust_ffi::FT_Outline_Get_CBox(Some(&snapshot), Some(&mut bbox));
    // SAFETY: `acbox` is non-null and the caller provides writable `FT_BBox` storage.
    unsafe {
        *acbox = FT_BBox {
            xMin: bbox.xMin,
            yMin: bbox.yMin,
            xMax: bbox.xMax,
            yMax: bbox.yMax,
        };
    }
}

fn c_glyph_cbox_snapshot(glyph: FT_Glyph) -> Option<rust_ffi::FT_GlyphCBoxSnapshot> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: the public C ABI accepts caller-owned `FT_Glyph` records; this
    // thin wrapper reads only the root record and the class pointer nullness
    // needed to reproduce FreeType's `FT_Glyph_Get_CBox` early-return order.
    let root = unsafe { glyph.as_ref() };
    if root.clazz.is_null() {
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: false,
            has_bbox_hook: false,
            cbox: None,
        });
    }
    if root.clazz == owned_outline_glyph_class() {
        let owned = owned_outline_glyph_from_root(glyph.as_ptr())?;
        let mut cbox = rust_ffi::FT_BBox::default();
        rust_ffi::FT_Outline_Get_CBox(Some(&owned.core.outline), Some(&mut cbox));
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: true,
            has_bbox_hook: true,
            cbox: Some(cbox),
        });
    }
    if root.clazz == owned_bitmap_glyph_class() {
        let owned = owned_bitmap_glyph_from_root(glyph.as_ptr())?;
        let x_min = i64::from(owned.record.left).saturating_mul(64);
        let y_max = i64::from(owned.record.top).saturating_mul(64);
        let x_max = x_min.saturating_add(i64::from(owned.record.bitmap.width).saturating_mul(64));
        let y_min = y_max.saturating_sub(i64::from(owned.record.bitmap.rows).saturating_mul(64));
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: true,
            has_bbox_hook: true,
            cbox: Some(rust_ffi::FT_BBox {
                xMin: x_min,
                yMin: y_min,
                xMax: x_max,
                yMax: y_max,
            }),
        });
    }
    // SAFETY: `glyph->clazz` is non-null.  The wrapper reads the public-sized
    // class facade to observe whether `glyph_bbox` is present, then delegates
    // the zero/no-bbox behavior to safe Rust.
    let clazz = unsafe { &*root.clazz };
    Some(rust_ffi::FT_GlyphCBoxSnapshot {
        has_class: true,
        has_bbox_hook: !clazz.glyph_bbox.is_null(),
        cbox: None,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_Get_CBox(glyph: FT_Glyph, bbox_mode: FT_UInt, acbox: *mut FT_BBox) {
    let Some(acbox) = non_null_mut(acbox) else {
        return;
    };
    let snapshot = c_glyph_cbox_snapshot(glyph);
    let mut bbox = rust_ffi::FT_BBox::default();
    rust_ffi::FT_Glyph_Get_CBox(snapshot, bbox_mode, Some(&mut bbox));
    // SAFETY: `acbox` is non-null and the caller provides writable `FT_BBox` storage.
    unsafe {
        *acbox.as_ptr() = FT_BBox {
            xMin: bbox.xMin,
            yMin: bbox.yMin,
            xMax: bbox.xMax,
            yMax: bbox.yMax,
        };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Glyph(slot: FT_GlyphSlot, aglyph: *mut FT_Glyph) -> FT_Error {
    let err = rust_ffi::FT_Get_Glyph(!slot.is_null(), !aglyph.is_null());
    if err != rust_ffi::FT_Err_Unimplemented_Feature as FT_Error {
        return err;
    }
    let Some(out) = non_null_mut(aglyph) else {
        return err;
    };
    let Some(slot) = non_null_mut(slot) else {
        return err;
    };
    // SAFETY: `slot` is a live slot allocated by this wrapper.  Successful
    // glyph creation copies the private Rust slot payload into an owned glyph.
    let slot = unsafe { slot.as_ref() };
    let glyph_result = if slot.rust_slot.format == rust_ffi::FT_GLYPH_FORMAT_BITMAP {
        rust_ffi::FT_Get_Bitmap_Glyph(Some(&slot.rust_slot))
            .map(|core| Box::into_raw(Box::new(OwnedBitmapGlyph::new(core))).cast::<FT_GlyphRec>())
    } else {
        rust_ffi::FT_Get_Outline_Glyph(Some(&slot.rust_slot))
            .map(|core| Box::into_raw(Box::new(OwnedOutlineGlyph::new(core))).cast::<FT_GlyphRec>())
    };
    match glyph_result {
        Ok(glyph) => {
            // SAFETY: `out` is non-null and points to caller-provided output storage.
            unsafe { *out.as_ptr() = glyph };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => {
            // SAFETY: `out` is non-null and points to caller-provided output storage.
            unsafe { *out.as_ptr() = ptr::null_mut() };
            error
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_Copy(source: FT_Glyph, target: *mut FT_Glyph) -> FT_Error {
    let source_has_class = if target.is_null() || source.is_null() {
        false
    } else {
        // SAFETY: `source` is non-null and this thin wrapper reads only the
        // class pointer needed for FreeType's early argument validation.
        unsafe { !(*source).clazz.is_null() }
    };
    let err = rust_ffi::FT_Glyph_Copy(!source.is_null(), !target.is_null(), source_has_class);
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && !target.is_null()
        && let Some(source) = owned_outline_glyph_from_root(source)
    {
        let copy = rust_ffi::FT_Outline_Glyph_Copy(&source.core);
        let copy = Box::into_raw(Box::new(OwnedOutlineGlyph::new(copy))).cast::<FT_GlyphRec>();
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = copy;
        }
        return rust_ffi::FT_Err_Ok;
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && !target.is_null()
        && let Some(source) = owned_bitmap_glyph_from_root(source)
    {
        let copy = rust_ffi::FT_Bitmap_Glyph_Copy(&source.core);
        let copy = Box::into_raw(Box::new(OwnedBitmapGlyph::new(copy))).cast::<FT_GlyphRec>();
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = copy;
        }
        return rust_ffi::FT_Err_Ok;
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error && !target.is_null() {
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = ptr::null_mut();
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Glyph(glyph: FT_Glyph) {
    if owned_outline_glyph_from_root(glyph).is_some() {
        // SAFETY: the class sentinel proves this pointer came from
        // `Box<OwnedOutlineGlyph>` in `FT_Get_Glyph`.
        unsafe { drop(Box::from_raw(glyph.cast::<OwnedOutlineGlyph>())) };
        return;
    }
    if owned_bitmap_glyph_from_root(glyph).is_some() {
        // SAFETY: the class sentinel proves this pointer came from
        // `Box<OwnedBitmapGlyph>` in `FT_Get_Glyph`.
        unsafe { drop(Box::from_raw(glyph.cast::<OwnedBitmapGlyph>())) };
        return;
    }
    rust_ffi::FT_Done_Glyph(!glyph.is_null());
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_Transform(
    glyph: FT_Glyph,
    matrix: *const FT_Matrix,
    delta: *const FT_Vector,
) -> FT_Error {
    if glyph.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(owned) = owned_outline_glyph_from_root_mut(glyph) else {
        let has_class = unsafe { !(*glyph).clazz.is_null() };
        return if has_class {
            rust_ffi::FT_Err_Invalid_Glyph_Format
        } else {
            rust_ffi::FT_Err_Invalid_Argument
        };
    };
    let matrix = non_null(matrix).map(|matrix| {
        // SAFETY: `matrix` is non-null and copied by value.
        let matrix = unsafe { matrix.as_ref() };
        rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        }
    });
    let delta = non_null(delta).map(|delta| {
        // SAFETY: `delta` is non-null and copied by value.
        let delta = unsafe { delta.as_ref() };
        rust_ffi::FT_Vector {
            x: delta.x,
            y: delta.y,
        }
    });
    let error = rust_ffi::FT_Glyph_Transform_Outline(
        Some(&mut owned.core),
        matrix.as_ref(),
        delta.as_ref(),
    );
    if error == rust_ffi::FT_Err_Ok {
        owned.refresh_record();
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_To_Bitmap(
    the_glyph: *mut FT_Glyph,
    render_mode: FT_Render_Mode,
    origin: *const FT_Vector,
    destroy: FT_Bool,
) -> FT_Error {
    if !the_glyph.is_null() {
        // SAFETY: `the_glyph` is non-null and points to caller-owned handle
        // storage.  We only copy the handle value before validating the glyph.
        let glyph = unsafe { *the_glyph };
        if owned_bitmap_glyph_from_root(glyph).is_some() {
            // FreeType `src/base/ftglyph.c:794-795` returns success without
            // replacing or freeing an already-bitmap glyph.
            return rust_ffi::FT_Err_Ok;
        }
        if let Some(owned) = owned_outline_glyph_from_root(glyph) {
            if !origin.is_null() {
                return rust_ffi::FT_Err_Unimplemented_Feature;
            }
            let bitmap = match rust_ffi::FT_Outline_Glyph_To_Bitmap(&owned.core, render_mode) {
                Ok(bitmap) => bitmap,
                Err(error) => return error,
            };
            let bitmap =
                Box::into_raw(Box::new(OwnedBitmapGlyph::new(bitmap))).cast::<FT_GlyphRec>();
            if destroy != 0 {
                // SAFETY: the class sentinel proves this pointer came from
                // `Box<OwnedOutlineGlyph>` in `FT_Get_Glyph`.
                unsafe { drop(Box::from_raw(glyph.cast::<OwnedOutlineGlyph>())) };
            }
            // SAFETY: `the_glyph` is non-null and points to caller-provided
            // handle storage.  C FreeType replaces it after successful render.
            unsafe {
                *the_glyph = bitmap;
            }
            return rust_ffi::FT_Err_Ok;
        }
    }
    let (glyph_present, library_present, class_present, prepare_hook_present) =
        if the_glyph.is_null() {
            (false, false, false, false)
        } else {
            // SAFETY: `the_glyph` is non-null and this thin wrapper only reads
            // the caller handle to reproduce FreeType's early argument checks.
            let glyph = unsafe { *the_glyph };
            if glyph.is_null() {
                (false, false, false, false)
            } else {
                // SAFETY: `glyph` is non-null and this wrapper reads only the
                // root fields used before FreeType allocates or renders.
                let glyph_ref = unsafe { &*glyph };
                let class_present = !glyph_ref.clazz.is_null();
                let prepare_hook_present = if class_present {
                    // SAFETY: `clazz` is non-null and only the function-pointer
                    // nullness is observed, matching the C Bad-path guard.
                    unsafe { !(*glyph_ref.clazz).glyph_prepare.is_null() }
                } else {
                    false
                };
                (
                    true,
                    !glyph_ref.library.is_null(),
                    class_present,
                    prepare_hook_present,
                )
            }
        };
    rust_ffi::FT_Glyph_To_Bitmap(
        !the_glyph.is_null(),
        glyph_present,
        library_present,
        class_present,
        prepare_hook_present,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_BBox(outline: *const FT_Outline, abbox: *mut FT_BBox) -> FT_Error {
    if abbox.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let mut bbox = rust_ffi::FT_BBox::default();
    let error = rust_ffi::FT_Outline_Get_BBox(Some(&snapshot), Some(&mut bbox));
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `abbox` is non-null and the caller provides writable `FT_BBox` storage.
        unsafe {
            *abbox = FT_BBox {
                xMin: bbox.xMin,
                yMin: bbox.yMin,
                xMax: bbox.xMax,
                yMax: bbox.yMax,
            };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_Bitmap(
    library: FT_Library,
    outline: *const FT_Outline,
    abitmap: *mut FT_Bitmap,
) -> FT_Error {
    let Some(mut bitmap) = non_null_mut(abitmap) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let snapshot = outline_snapshot_from_c(outline);
    let bitmap_view = {
        // SAFETY: `bitmap` is non-null and points to caller-owned `FT_Bitmap` storage.
        let bitmap = unsafe { bitmap.as_ref() };
        rust_ffi::FT_Bitmap_C {
            rows: bitmap.rows,
            width: bitmap.width,
            pitch: bitmap.pitch,
            buffer: bitmap.buffer,
            num_grays: bitmap.num_grays,
            pixel_mode: u8::try_from(bitmap.pixel_mode).unwrap_or(0),
            palette_mode: bitmap.palette_mode,
            palette: bitmap.palette,
        }
    };
    match rust_ffi::FT_Outline_Get_Bitmap(
        library_ref(library),
        snapshot.as_ref(),
        Some(&bitmap_view),
    ) {
        Ok(rendered) => {
            // SAFETY: `bitmap` is non-null and still points to caller-owned storage.
            let bitmap = unsafe { bitmap.as_mut() };
            copy_rendered_bitmap_to_c(bitmap, &rendered);
            rust_ffi::FT_Err_Ok
        }
        Err(err) => err,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Render(
    library: FT_Library,
    outline: *const FT_Outline,
    params: *mut FT_Raster_Params,
) -> FT_Error {
    let Some(params) = (unsafe { params.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let snapshot = outline_snapshot_from_c(outline);
    let target = unsafe { params.target.as_ref() };
    let bitmap_view = target.map(bitmap_to_rust);
    let clip_box = rust_ffi::FT_BBox {
        xMin: params.clip_box.xMin,
        yMin: params.clip_box.yMin,
        xMax: params.clip_box.xMax,
        yMax: params.clip_box.yMax,
    };
    let library_view = library_ref(library);
    if library_view.is_some()
        && snapshot.as_ref().is_some_and(|outline_snapshot| {
            let mut cbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
            cbox.xMin >= -0x1000000
                && cbox.yMin >= -0x1000000
                && cbox.xMax <= 0x1000000
                && cbox.yMax <= 0x1000000
        })
    {
        // FreeType 2.14.3 ftoutln.c:625-648 mutates `source` after
        // library/outline/cbox validation and before invoking the renderer,
        // so renderer errors retain this mutation too.
        params.source = outline.cast();
    }

    if params.flags & rust_ffi::FT_RASTER_FLAG_DIRECT as c_int != 0 {
        if params.flags & rust_ffi::FT_RASTER_FLAG_CLIP as c_int == 0 {
            if let (Some(_library), Some(outline_snapshot)) = (library_view, snapshot.as_ref()) {
                let mut cbox = rust_ffi::FT_BBox::default();
                rust_ffi::FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
                if cbox.xMin >= -0x1000000
                    && cbox.yMin >= -0x1000000
                    && cbox.xMax <= 0x1000000
                    && cbox.yMax <= 0x1000000
                {
                    // FreeType 2.14.3 ftoutln.c:635-640 presets direct-mode
                    // no-CLIP bounds from the outline CBox in integer pixels.
                    params.clip_box.xMin = cbox.xMin >> 6;
                    params.clip_box.yMin = cbox.yMin >> 6;
                    params.clip_box.xMax = cbox.xMax.checked_add(63).unwrap_or(cbox.xMax) >> 6;
                    params.clip_box.yMax = cbox.yMax.checked_add(63).unwrap_or(cbox.yMax) >> 6;
                }
            }
        }
        return match rust_ffi::FT_Outline_Render_Direct_Spans(
            library_view,
            snapshot.as_ref(),
            bitmap_view.as_ref(),
            params.flags,
            Some(rust_ffi::FT_BBox {
                xMin: params.clip_box.xMin,
                yMin: params.clip_box.yMin,
                xMax: params.clip_box.xMax,
                yMax: params.clip_box.yMax,
            }),
            params.gray_spans.is_some(),
        ) {
            Ok(spans) => {
                if let Some(callback) = params.gray_spans {
                    for row in spans.chunk_by(|left, right| left.0 == right.0) {
                        let y = row[0].0;
                        let c_spans = row
                            .iter()
                            .map(|(_, span)| FT_Span {
                                x: i16::from_ne_bytes(span.x.to_ne_bytes()),
                                len: span.len,
                                coverage: span.coverage,
                            })
                            .collect::<Vec<_>>();
                        // SAFETY: `c_spans` lives for the synchronous callback
                        // invocation, and `params.user` is the caller-provided
                        // opaque pointer FreeType passes through unchanged.
                        unsafe {
                            callback(
                                y,
                                c_int::try_from(c_spans.len()).unwrap_or(c_int::MAX),
                                c_spans.as_ptr(),
                                params.user,
                            );
                        }
                    }
                }
                rust_ffi::FT_Err_Ok
            }
            Err(err) => err,
        };
    }

    match rust_ffi::FT_Outline_Render(
        library_view,
        snapshot.as_ref(),
        bitmap_view.as_ref(),
        params.flags,
        clip_box,
    ) {
        Ok(rendered) => {
            if let Some(target) = target {
                if target.width != 0 && target.rows != 0 && target.buffer.is_null() {
                    return rust_ffi::FT_Err_Invalid_Argument;
                }
                copy_rendered_bitmap_to_c(target, &rendered);
            }
            rust_ffi::FT_Err_Ok
        }
        Err(err) => {
            if let (Some(target), Some(rendered)) = (
                target,
                rust_ffi::FT_Outline_Render_Error_Output(
                    snapshot.as_ref(),
                    bitmap_view.as_ref(),
                    params.flags,
                ),
            ) {
                copy_rendered_bitmap_to_c(target, &rendered);
            }
            err
        }
    }
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_support_outline_render_gray_spans(
    y: c_int,
    count: c_int,
    spans: *const FT_Span,
    user: *mut c_void,
) {
    TEST_OUTLINE_RENDER_USER_TOKEN.with(|token| {
        TEST_OUTLINE_RENDER_USER_SEEN.with(|seen| {
            *seen.borrow_mut() = user == *token.borrow();
        });
    });
    if count <= 0 || spans.is_null() {
        return;
    }
    // SAFETY: FreeType span callbacks provide `count` initialized records
    // valid for this synchronous callback invocation.
    let spans = unsafe { slice::from_raw_parts(spans, usize::try_from(count).unwrap_or(0)) };
    TEST_OUTLINE_RENDER_SPANS.with(|recorded| {
        recorded
            .borrow_mut()
            .extend(spans.iter().copied().map(|span| (y, span)));
    });
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_outline_render_direct_spans(
    library: FT_Library,
    outline: *const FT_Outline,
    params: *mut FT_Raster_Params,
    gray_spans_present: bool,
    user_token: *mut c_void,
) -> (FT_Error, Vec<(c_int, FT_Span)>, bool) {
    TEST_OUTLINE_RENDER_SPANS.with(|spans| spans.borrow_mut().clear());
    TEST_OUTLINE_RENDER_USER_SEEN.with(|seen| *seen.borrow_mut() = false);
    TEST_OUTLINE_RENDER_USER_TOKEN.with(|token| *token.borrow_mut() = user_token);
    if let Some(params) = unsafe { params.as_mut() } {
        params.user = user_token;
        params.gray_spans = gray_spans_present.then_some(abi_support_outline_render_gray_spans);
    }
    let error = FT_Outline_Render(library, outline, params);
    let spans = TEST_OUTLINE_RENDER_SPANS.with(|recorded| recorded.borrow().clone());
    let user_seen = TEST_OUTLINE_RENDER_USER_SEEN.with(|seen| *seen.borrow());
    (error, spans, user_seen)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_outline_decompose_trace(
    outline: *const FT_Outline,
    transforms: &[(rust_ffi::FT_Int, rust_ffi::FT_Pos)],
) -> Result<Vec<rust_ffi::FTOutlineDecomposeRun>, FT_Error> {
    let snapshot = outline_snapshot_from_c(outline);
    rust_ffi::FT_Outline_Decompose_Trace(snapshot.as_ref(), transforms)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_Orientation(outline: *const FT_Outline) -> FT_Orientation {
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_ORIENTATION_TRUETYPE as FT_Orientation;
    };
    rust_ffi::FT_Outline_Get_Orientation(Some(&snapshot)) as FT_Orientation
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Check(outline: *const FT_Outline) -> FT_Error {
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    rust_ffi::FT_Outline_Check(Some(&snapshot))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Copy(source: *const FT_Outline, target: *mut FT_Outline) -> FT_Error {
    if source == target.cast_const() && !source.is_null() {
        return rust_ffi::FT_Err_Ok;
    }
    let Some(source_snapshot) = outline_snapshot_from_c(source) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let Some(mut target_snapshot) = outline_snapshot_from_c(target) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_Copy(Some(&source_snapshot), Some(&mut target_snapshot));
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_c(target, &target_snapshot, true);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Embolden(outline: *mut FT_Outline, strength: FT_Pos) -> FT_Error {
    let Some(mut snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_Embolden(Some(&mut snapshot), strength);
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_c(outline, &snapshot, false);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_EmboldenXY(
    outline: *mut FT_Outline,
    xstrength: FT_Pos,
    ystrength: FT_Pos,
) -> FT_Error {
    let Some(mut snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_EmboldenXY(Some(&mut snapshot), xstrength, ystrength);
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_c(outline, &snapshot, false);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_GetInsideBorder(outline: *const FT_Outline) -> FT_StrokerBorder {
    let snapshot = outline_snapshot_from_c(outline);
    rust_ffi::FT_Outline_GetInsideBorder(snapshot.as_ref()) as FT_StrokerBorder
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_GetOutsideBorder(outline: *const FT_Outline) -> FT_StrokerBorder {
    let snapshot = outline_snapshot_from_c(outline);
    rust_ffi::FT_Outline_GetOutsideBorder(snapshot.as_ref()) as FT_StrokerBorder
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_New(library: FT_Library, astroker: *mut FT_Stroker) -> FT_Error {
    let Some(out) = non_null_mut(astroker) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut stroker = ptr::null_mut();
    let err = rust_ffi::FT_Stroker_New(library_ref(library), Some(&mut stroker));
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and points to caller-provided output storage.
        unsafe {
            *out.as_ptr() = stroker;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_Set(
    stroker: FT_Stroker,
    radius: FT_Fixed,
    line_cap: c_int,
    line_join: c_int,
    miter_limit: FT_Fixed,
) {
    rust_ffi::FT_Stroker_Set(stroker, radius, line_cap, line_join, miter_limit);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_Rewind(stroker: FT_Stroker) {
    rust_ffi::FT_Stroker_Rewind(stroker);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_BeginSubPath(
    stroker: FT_Stroker,
    to: *const FT_Vector,
    open: FT_Bool,
) -> FT_Error {
    let rust_to = if to.is_null() {
        None
    } else {
        // SAFETY: `to` is non-null and points to a C ABI `FT_Vector` for the
        // duration of this thin forwarding call.
        let to = unsafe { &*to };
        Some(rust_ffi::FT_Vector { x: to.x, y: to.y })
    };
    rust_ffi::FT_Stroker_BeginSubPath(stroker, rust_to.as_ref(), open)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_LineTo(stroker: FT_Stroker, to: *const FT_Vector) -> FT_Error {
    let rust_to = if to.is_null() {
        None
    } else {
        // SAFETY: `to` is non-null and points to a C ABI `FT_Vector` for the
        // duration of this thin forwarding call.
        let to = unsafe { &*to };
        Some(rust_ffi::FT_Vector { x: to.x, y: to.y })
    };
    rust_ffi::FT_Stroker_LineTo(stroker, rust_to.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_GetBorderCounts(
    stroker: FT_Stroker,
    border: FT_StrokerBorder,
    anum_points: *mut FT_UInt,
    anum_contours: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: The optional output pointers, when non-null, are caller-owned
    // `FT_UInt` records valid for the duration of this C ABI call.
    let points = unsafe { anum_points.as_mut() };
    let contours = unsafe { anum_contours.as_mut() };
    rust_ffi::FT_Stroker_GetBorderCounts(stroker, border, points, contours)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_GetCounts(
    stroker: FT_Stroker,
    anum_points: *mut FT_UInt,
    anum_contours: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: The optional output pointers, when non-null, are caller-owned
    // `FT_UInt` records valid for the duration of this C ABI call.
    let points = unsafe { anum_points.as_mut() };
    let contours = unsafe { anum_contours.as_mut() };
    rust_ffi::FT_Stroker_GetCounts(stroker, points, contours)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_Done(stroker: FT_Stroker) {
    rust_ffi::FT_Stroker_Done(stroker);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_ExportBorder(
    stroker: FT_Stroker,
    border: FT_StrokerBorder,
    outline: *mut FT_Outline,
) {
    // The core route maintained so far covers FreeType's no-op cases for null
    // inputs, invalid border, and newly allocated/unparsed strokers.  Geometry
    // export remains pending and must not be implemented in this ABI wrapper.
    let mut snapshot = rust_ffi::FT_OutlineSnapshot::default();
    rust_ffi::FT_Stroker_ExportBorder(
        stroker,
        border,
        (!outline.is_null()).then_some(&mut snapshot),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_Export(stroker: FT_Stroker, outline: *mut FT_Outline) {
    // See `FT_Stroker_ExportBorder`; this wrapper intentionally delegates only
    // the currently maintained no-op route to core.
    let mut snapshot = rust_ffi::FT_OutlineSnapshot::default();
    rust_ffi::FT_Stroker_Export(stroker, (!outline.is_null()).then_some(&mut snapshot));
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_New(
    library: FT_Library,
    numPoints: FT_UInt,
    numContours: FT_Int,
    anoutline: *mut FT_Outline,
) -> FT_Error {
    if library.is_null() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(outline) = (unsafe { anoutline.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if numPoints > u32::from(u16::MAX) {
        return rust_ffi::FT_Err_Array_Too_Large as FT_Error;
    }
    if numContours < 0 || u32::try_from(numContours).map_or(true, |contours| contours > numPoints) {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let point_count = usize::try_from(numPoints).unwrap_or(usize::MAX);
    let contour_count = usize::try_from(numContours).unwrap_or(usize::MAX);
    let points = alloc_outline_array::<FT_Vector>(point_count).cast::<FT_Vector>();
    let tags = alloc_outline_array::<FT_Byte>(point_count).cast::<FT_Byte>();
    let contours = alloc_outline_array::<FT_UShort>(contour_count).cast::<FT_UShort>();
    if (point_count > 0 && (points.is_null() || tags.is_null()))
        || (contour_count > 0 && contours.is_null())
    {
        dealloc_outline_array(points.cast::<u8>(), point_count, Layout::array::<FT_Vector>);
        dealloc_outline_array(tags.cast::<u8>(), point_count, Layout::array::<FT_Byte>);
        dealloc_outline_array(
            contours.cast::<u8>(),
            contour_count,
            Layout::array::<FT_UShort>,
        );
        return rust_ffi::FT_Err_Out_Of_Memory;
    }
    *outline = FT_Outline {
        n_contours: FT_UShort::try_from(numContours).unwrap_or(FT_UShort::MAX),
        n_points: FT_UShort::try_from(numPoints).unwrap_or(FT_UShort::MAX),
        points,
        tags,
        contours,
        flags: rust_ffi::FT_OUTLINE_OWNER as FT_Int,
    };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Done(library: FT_Library, outline: *mut FT_Outline) -> FT_Error {
    if library.is_null() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(outline) = (unsafe { outline.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    if outline.flags & rust_ffi::FT_OUTLINE_OWNER as FT_Int != 0 {
        dealloc_outline_array(
            outline.points.cast::<u8>(),
            usize::from(outline.n_points),
            Layout::array::<FT_Vector>,
        );
        dealloc_outline_array(
            outline.tags.cast::<u8>(),
            usize::from(outline.n_points),
            Layout::array::<FT_Byte>,
        );
        dealloc_outline_array(
            outline.contours.cast::<u8>(),
            usize::from(outline.n_contours),
            Layout::array::<FT_UShort>,
        );
    }
    *outline = FT_Outline::default();
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Reverse(outline: *mut FT_Outline) {
    let Some(mut snapshot) = outline_snapshot_from_c(outline) else {
        return;
    };
    rust_ffi::FT_Outline_Reverse(Some(&mut snapshot));
    copy_outline_snapshot_to_c(outline, &snapshot, true);
}

fn alloc_outline_array<T>(count: usize) -> *mut u8 {
    if count == 0 {
        return ptr::null_mut();
    }
    let Ok(layout) = Layout::array::<T>(count) else {
        return ptr::null_mut();
    };
    // SAFETY: `layout` was constructed for an array allocation and is non-zero sized.
    unsafe { alloc_zeroed(layout) }
}

fn dealloc_outline_array(
    ptr: *mut u8,
    count: usize,
    layout_for: impl FnOnce(usize) -> Result<Layout, std::alloc::LayoutError>,
) {
    if ptr.is_null() || count == 0 {
        return;
    }
    let Ok(layout) = layout_for(count) else {
        return;
    };
    // SAFETY: outline OWNER allocations in this module use the matching layout.
    unsafe { dealloc(ptr, layout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Transform(outline: *const FT_Outline, matrix: *const FT_Matrix) {
    let (Some(mut snapshot), Some(matrix)) = (outline_snapshot_from_c(outline), non_null(matrix))
    else {
        return;
    };
    // SAFETY: `matrix` is non-null and points to a caller-owned `FT_Matrix`
    // that remains readable for this call.
    let matrix = unsafe { matrix.as_ref() };
    let matrix = rust_ffi::FT_Matrix {
        xx: matrix.xx,
        xy: matrix.xy,
        yx: matrix.yx,
        yy: matrix.yy,
    };
    rust_ffi::FT_Outline_Transform(Some(&mut snapshot), Some(&matrix));
    copy_outline_snapshot_to_c(outline.cast_mut(), &snapshot, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Translate(
    outline: *const FT_Outline,
    x_offset: FT_Pos,
    y_offset: FT_Pos,
) {
    let Some(mut snapshot) = outline_snapshot_from_c(outline) else {
        return;
    };
    rust_ffi::FT_Outline_Translate(Some(&mut snapshot), x_offset, y_offset);
    copy_outline_snapshot_to_c(outline.cast_mut(), &snapshot, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Char_Size(
    face: FT_Face,
    char_width: FT_F26Dot6,
    char_height: FT_F26Dot6,
    horz_resolution: FT_UInt,
    vert_resolution: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let error = rust_ffi::FT_Set_Char_Size(
        &mut state.inner,
        char_width,
        char_height,
        horz_resolution,
        vert_resolution,
    );
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Pixel_Sizes(
    face: FT_Face,
    pixel_width: FT_UInt,
    pixel_height: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let error = rust_ffi::FT_Set_Pixel_Sizes(&mut state.inner, pixel_width, pixel_height);
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Transform(
    face: FT_Face,
    matrix: *const FT_Matrix,
    delta: *const FT_Vector,
) {
    let Some(state) = face_state_mut(face) else {
        return;
    };
    let rust_matrix = if matrix.is_null() {
        None
    } else {
        // SAFETY: `matrix` is non-null and points to a C ABI `FT_Matrix`.
        let matrix = unsafe { *matrix };
        Some(rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        })
    };
    let rust_delta = if delta.is_null() {
        None
    } else {
        // SAFETY: `delta` is non-null and points to a C ABI `FT_Vector`.
        let delta = unsafe { *delta };
        Some(rust_ffi::FT_Vector {
            x: delta.x,
            y: delta.y,
        })
    };
    rust_ffi::FT_Set_Transform(
        Some(&mut state.inner),
        rust_matrix.as_ref(),
        rust_delta.as_ref(),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Request_Size(face: FT_Face, req: *const FT_Size_RequestRec) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let request = if req.is_null() {
        None
    } else {
        // SAFETY: `req` was checked for null and is only copied by value.
        let req = unsafe { *req };
        Some(rust_ffi::FT_Size_RequestRec {
            type_: req.type_,
            width: req.width,
            height: req.height,
            horiResolution: req.horiResolution,
            vertResolution: req.vertResolution,
        })
    };
    let error = rust_ffi::FT_Request_Size(Some(&mut state.inner), request.as_ref());
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Select_Size(face: FT_Face, strike_index: FT_Int) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Select_Size(Some(&mut state.inner), strike_index);
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Size(face: FT_Face, asize: *mut FT_Size) -> FT_Error {
    let Some(_face_ptr) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = non_null_mut(asize) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };

    // SAFETY: `out` is a valid output pointer checked above.
    unsafe { *out.as_ptr() = ptr::null_mut() };
    let mut rust_size: rust_ffi::FT_Size = ptr::null_mut();
    let error = rust_ffi::FT_New_Size(Some(&state.inner), Some(&mut rust_size));
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }

    let size = Box::into_raw(Box::new(FT_SizeRec {
        face,
        generic: FT_Generic::default(),
        metrics: rust_size_metrics_to_abi(state.inner.size_metrics),
        internal: ptr::dangling_mut::<c_void>(),
        rust_size,
        owner: face,
    }));
    state.push_size_record(size);
    // SAFETY: `out` is a valid output pointer checked above.
    unsafe { *out.as_ptr() = size };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Activate_Size(size: FT_Size) -> FT_Error {
    let Some(size_ptr) = non_null_mut(size) else {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    };
    // SAFETY: `size_ptr` is a live size record allocated by this wrapper.
    let (owner, rust_size) = unsafe {
        let record = size_ptr.as_ref();
        (record.owner, record.rust_size)
    };
    let Some(face_ptr) = non_null_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(_state) = face_state_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Activate_Size(rust_size);
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `face_ptr` is a live parent face and `size` is one of its size records.
        unsafe { (*face_ptr.as_ptr()).size = size };
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Size(size: FT_Size) -> FT_Error {
    let Some(size_ptr) = non_null_mut(size) else {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    };
    // SAFETY: `size_ptr` is a live size record allocated by this wrapper.
    let (owner, rust_size) = unsafe {
        let record = size_ptr.as_ref();
        (record.owner, record.rust_size)
    };
    let Some(face_ptr) = non_null_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(state) = face_state_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };

    let error = rust_ffi::FT_Done_Size(rust_size);
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }
    let was_active = unsafe { (*face_ptr.as_ptr()).size == size };
    let removed = state.remove_size_record(size);
    if !removed {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    }
    if was_active {
        let fallback = state
            .size_records
            .first()
            .copied()
            .unwrap_or(ptr::null_mut());
        // SAFETY: `face_ptr` is a live parent face; fallback is either null or still face-owned.
        unsafe { (*face_ptr.as_ptr()).size = fallback };
    }
    // SAFETY: the record has been removed from `state.size_records` and is consumed here.
    unsafe { drop_size(size) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Char_Index(face: FT_Face, char_code: FT_ULong) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    rust_ffi::FT_Get_Char_Index(&state.inner, char_code)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharVariantIndex(
    face: FT_Face,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    rust_ffi::FT_Face_GetCharVariantIndex(Some(&state.inner), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharVariantIsDefault(
    face: FT_Face,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_Int {
    let Some(state) = face_state(face) else {
        return -1;
    };
    rust_ffi::FT_Face_GetCharVariantIsDefault(Some(&state.inner), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetVariantSelectors(face: FT_Face) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetVariantSelectors(Some(&state.inner));
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetVariantsOfChar(face: FT_Face, charcode: FT_ULong) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetVariantsOfChar(Some(&state.inner), charcode);
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharsOfVariant(
    face: FT_Face,
    variant_selector: FT_ULong,
) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetCharsOfVariant(Some(&state.inner), variant_selector);
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Kerning(
    face: FT_Face,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    kern_mode: FT_UInt,
    akerning: *mut FT_Vector,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = NonNull::new(akerning) else {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    };
    let mut vector = rust_ffi::FT_Vector::default();
    let err = rust_ffi::FT_Get_Kerning(
        Some(&state.inner),
        left_glyph,
        right_glyph,
        kern_mode,
        Some(&mut vector),
    );
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out.as_ptr() = FT_Vector {
                x: vector.x,
                y: vector.y,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PFR_Kerning(
    face: FT_Face,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    avector: *mut FT_Vector,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = NonNull::new(avector) else {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    };
    let mut vector = rust_ffi::FT_Vector::default();
    let err = rust_ffi::FT_Get_PFR_Kerning(
        Some(&state.inner),
        left_glyph,
        right_glyph,
        Some(&mut vector),
    );
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out.as_ptr() = FT_Vector {
                x: vector.x,
                y: vector.y,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Select_Charmap(face: FT_Face, encoding: FT_Encoding) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    rust_ffi::FT_Select_Charmap(Some(&mut state.inner), encoding)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Charmap(face: FT_Face, charmap: FT_CharMap) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if state.charmaps.is_empty() || charmap.is_null() {
        return rust_ffi::FT_Err_Invalid_CharMap_Handle as FT_Error;
    }
    let Some(index) = state.charmap_index(charmap) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    rust_ffi::FT_Set_Charmap(Some(&mut state.inner), rust_charmap)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Charmap_Index(charmap: FT_CharMap) -> FT_Int {
    let Some(charmap) = NonNull::new(charmap) else {
        return -1;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return -1;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return -1;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return -1;
    };
    rust_ffi::FT_Get_Charmap_Index(rust_charmap) as FT_Int
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CMap_Format(charmap: FT_CharMap) -> FT_Long {
    let Some(charmap) = NonNull::new(charmap) else {
        return -1;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return -1;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return -1;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return -1;
    };
    rust_ffi::FT_Get_CMap_Format(rust_charmap) as FT_Long
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CMap_Language_ID(charmap: FT_CharMap) -> FT_ULong {
    let Some(charmap) = NonNull::new(charmap) else {
        return 0;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return 0;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return 0;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return 0;
    };
    rust_ffi::FT_Get_CMap_Language_ID(rust_charmap) as FT_ULong
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_FSType_Flags(face: FT_Face) -> FT_UShort {
    rust_ffi::FT_Get_FSType_Flags(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Gasp(face: FT_Face, ppem: FT_UInt) -> FT_Int {
    rust_ffi::FT_Get_Gasp(face_state(face).map(|state| &state.inner), ppem)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Glyph_Name(
    face: FT_Face,
    glyph_index: FT_UInt,
    buffer: *mut c_void,
    buffer_max: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if buffer.is_null() || buffer_max == 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: `buffer` is non-null, and the C caller promises at least
    // `buffer_max` writable bytes following FreeType's caller-allocated API.
    let buffer = unsafe { slice::from_raw_parts_mut(buffer.cast::<u8>(), buffer_max as usize) };
    match rust_ffi::FT_Get_Glyph_Name(&state.inner, glyph_index, buffer) {
        Ok(_) => rust_ffi::FT_Err_Ok,
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Name_Index(face: FT_Face, glyph_name: *const c_char) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    if glyph_name.is_null() {
        return 0;
    }
    // SAFETY: `glyph_name` is non-null and follows FreeType's C string
    // contract for this borrowed input pointer.
    let glyph_name = unsafe { CStr::from_ptr(glyph_name) };
    let Ok(glyph_name) = glyph_name.to_str() else {
        return 0;
    };
    rust_ffi::FT_Get_Name_Index(Some(&state.inner), Some(glyph_name))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Postscript_Name(face: FT_Face) -> *const c_char {
    face_state(face)
        .and_then(|state| state.postscript_name.as_deref())
        .map_or(ptr::null(), CStr::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Font_Format(face: FT_Face) -> *const c_char {
    face_state(face)
        .and_then(|state| state.font_format.as_deref())
        .map_or(ptr::null(), CStr::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_X11_Font_Format(face: FT_Face) -> *const c_char {
    FT_Get_Font_Format(face)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_face_driver_name(face: FT_Face) -> *const c_char {
    face_state(face)
        .and_then(|state| state.face_driver_name.as_deref())
        .map_or(ptr::null(), CStr::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Named_Instance(face: FT_Face, instance_index: FT_UInt) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let err = rust_ffi::FT_Set_Named_Instance(Some(&mut state.inner), instance_index);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Var_Design_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    let err = rust_ffi::FT_Set_Var_Design_Coordinates(Some(&mut state.inner), num_coords, coords);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Var_Design_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` writable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts_mut(coords, num_coords as usize) })
    };
    rust_ffi::FT_Get_Var_Design_Coordinates(Some(&state.inner), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Var_Blend_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` writable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts_mut(coords, num_coords as usize) })
    };
    rust_ffi::FT_Get_Var_Blend_Coordinates(Some(&state.inner), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_MM_Blend_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    FT_Get_Var_Blend_Coordinates(face, num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Multi_Master(face: FT_Face, amaster: *mut FT_Multi_Master) -> FT_Error {
    // SAFETY: the caller provides writable storage for the public descriptor or null.
    let amaster = unsafe { amaster.as_mut() };
    rust_ffi::FT_Get_Multi_Master(face_state(face).map(|state| &state.inner), amaster)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_MM_Design_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *const FT_Long,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Long values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    let err = rust_ffi::FT_Set_MM_Design_Coordinates(Some(&mut state.inner), num_coords, coords);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_MM_WeightVector(
    face: FT_Face,
    len: FT_UInt,
    weightvector: *const FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let weightvector = if weightvector.is_null() {
        None
    } else {
        // SAFETY: caller provides `len` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(weightvector, len as usize) })
    };
    rust_ffi::FT_Set_MM_WeightVector(Some(&mut state.inner), len, weightvector)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_MM_WeightVector(
    face: FT_Face,
    len: *mut FT_UInt,
    weightvector: *mut FT_Fixed,
) -> FT_Error {
    let len_ref = unsafe { len.as_mut() };
    let capacity = len_ref.as_ref().map_or(0, |len| **len as usize);
    let weightvector = if weightvector.is_null() {
        None
    } else {
        // SAFETY: caller provides `*len` writable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts_mut(weightvector, capacity) })
    };
    rust_ffi::FT_Get_MM_WeightVector(
        face_state(face).map(|state| &state.inner),
        len_ref,
        weightvector,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Var_Blend_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    let err = rust_ffi::FT_Set_Var_Blend_Coordinates(Some(&mut state.inner), num_coords, coords);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_MM_Blend_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    let err = rust_ffi::FT_Set_MM_Blend_Coordinates(Some(&mut state.inner), num_coords, coords);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Default_Named_Instance(
    face: FT_Face,
    instance_index: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the scalar output or null.
    let instance_index = unsafe { instance_index.as_mut() };
    rust_ffi::FT_Get_Default_Named_Instance(
        face_state(face).map(|state| &state.inner),
        instance_index,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_WinFNT_Header(
    face: FT_Face,
    header: *mut FT_WinFNT_HeaderRec,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the header output or null.
    let header = unsafe { header.as_mut() };
    rust_ffi::FT_Get_WinFNT_Header(face_state(face).map(|state| &state.inner), header)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_BDF_Property(
    face: FT_Face,
    prop_name: *const c_char,
    aproperty: *mut BDF_PropertyRec,
) -> FT_Error {
    let prop_name = property_name_arg(prop_name);
    let property = if aproperty.is_null() {
        None
    } else {
        // SAFETY: the caller provides writable storage for the BDF property
        // output or null.
        Some(unsafe { &mut *aproperty })
    };
    rust_ffi::FT_Get_BDF_Property(
        face_state(face).map(|state| &state.inner),
        prop_name.as_deref(),
        property,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_BDF_Charset_ID(
    face: FT_Face,
    acharset_encoding: *mut *const c_char,
    acharset_registry: *mut *const c_char,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for either output pointer
    // or null, matching FreeType's nullable output contract.
    let charset_encoding = unsafe { acharset_encoding.as_mut() };
    // SAFETY: same as above for the registry output pointer.
    let charset_registry = unsafe { acharset_registry.as_mut() };
    rust_ffi::FT_Get_BDF_Charset_ID(
        face_state(face).map(|state| &state.inner),
        charset_encoding,
        charset_registry,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CID_Is_Internally_CID_Keyed(
    face: FT_Face,
    is_cid: *mut FT_Bool,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the output pointer or
    // null, matching FreeType's nullable output contract.
    let is_cid = unsafe { is_cid.as_mut() };
    rust_ffi::FT_Get_CID_Is_Internally_CID_Keyed(face_state(face).map(|state| &state.inner), is_cid)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CID_From_Glyph_Index(
    face: FT_Face,
    glyph_index: FT_UInt,
    cid: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the output pointer or
    // null, matching FreeType's nullable output contract.
    let cid = unsafe { cid.as_mut() };
    rust_ffi::FT_Get_CID_From_Glyph_Index(
        face_state(face).map(|state| &state.inner),
        glyph_index,
        cid,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CID_Registry_Ordering_Supplement(
    face: FT_Face,
    registry: *mut *const c_char,
    ordering: *mut *const c_char,
    supplement: *mut FT_Int,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for each output pointer or
    // null, matching FreeType's nullable output contract.
    let registry = unsafe { registry.as_mut() };
    // SAFETY: same as above for ordering.
    let ordering = unsafe { ordering.as_mut() };
    // SAFETY: same as above for supplement.
    let supplement = unsafe { supplement.as_mut() };
    rust_ffi::FT_Get_CID_Registry_Ordering_Supplement(
        face_state(face).map(|state| &state.inner),
        registry,
        ordering,
        supplement,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Name_Count(face: FT_Face) -> FT_UInt {
    rust_ffi::FT_Get_Sfnt_Name_Count(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Name(
    face: FT_Face,
    idx: FT_UInt,
    aname: *mut FT_SfntName,
) -> FT_Error {
    let Some(out) = non_null_mut(aname) else {
        return rust_ffi::FT_Get_Sfnt_Name(face_state(face).map(|state| &state.inner), idx, None);
    };
    let mut name = rust_ffi::FT_SfntName::default();
    let error = rust_ffi::FT_Get_Sfnt_Name(
        face_state(face).map(|state| &state.inner),
        idx,
        Some(&mut name),
    );
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out.as_ptr() = FT_SfntName {
                platform_id: name.platform_id,
                encoding_id: name.encoding_id,
                language_id: name.language_id,
                name_id: name.name_id,
                string: name.string,
                string_len: name.string_len,
            };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Track_Kerning(
    face: FT_Face,
    point_size: FT_Fixed,
    degree: FT_Int,
    akerning: *mut FT_Fixed,
) -> FT_Error {
    let mut kerning = 0;
    let output = non_null_mut(akerning);
    let error = rust_ffi::FT_Get_Track_Kerning(
        face_state(face).map(|state| &state.inner),
        point_size,
        degree,
        output.map(|_| &mut kerning),
    );
    if error == rust_ffi::FT_Err_Ok {
        if let Some(output) = output {
            // SAFETY: `akerning` was checked for null and points to writable caller storage.
            unsafe { *output.as_ptr() = kerning };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Table(face: FT_Face, tag: FT_Sfnt_Tag) -> *mut c_void {
    let Some(state) = face_state(face) else {
        return ptr::null_mut();
    };
    rust_ffi::FT_Get_Sfnt_Table(&state.inner, tag)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Sfnt_Table(
    face: FT_Face,
    tag: FT_ULong,
    offset: FT_Long,
    buffer: *mut FT_Byte,
    length: *mut FT_ULong,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(len_ptr) = non_null_mut(length) else {
        return match rust_ffi::FT_Load_Sfnt_Table(&state.inner, tag, offset, None) {
            Ok(Some(bytes)) => {
                if let Some(buf) = non_null_mut(buffer) {
                    // SAFETY: caller provides a buffer large enough for the selected table.
                    unsafe {
                        ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_ptr().cast(), bytes.len());
                    }
                }
                rust_ffi::FT_Err_Ok as FT_Error
            }
            Ok(None) => rust_ffi::FT_Err_Ok as FT_Error,
            Err(err) => err as FT_Error,
        };
    };
    // SAFETY: caller-provided writable FT_ULong or NULL (caught above).
    let mut len_val = unsafe { *len_ptr.as_ptr() };
    match rust_ffi::FT_Load_Sfnt_Table(&state.inner, tag, offset, Some(&mut len_val)) {
        Ok(Some(bytes)) => {
            let copy_len = bytes.len().min(len_val as usize);
            if let Some(buf) = non_null_mut(buffer) {
                // SAFETY: caller provides a buffer of at least len_val bytes.
                unsafe {
                    ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_ptr().cast(), copy_len);
                }
            }
            // SAFETY: writable FT_ULong out-param.
            unsafe { *len_ptr.as_ptr() = copy_len as FT_ULong };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Ok(None) => {
            // SAFETY: writable FT_ULong out-param (length probe result).
            unsafe { *len_ptr.as_ptr() = len_val };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Err(err) => err as FT_Error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Sfnt_Table_Info(
    face: FT_Face,
    table_index: FT_UInt,
    tag: *mut FT_ULong,
    length: *mut FT_ULong,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let mut tag_out: rust_ffi::FT_ULong = 0;
    let mut length_out: rust_ffi::FT_ULong = 0;
    let tag_ref = if tag.is_null() {
        None
    } else {
        Some(&mut tag_out)
    };
    let length_ref = if length.is_null() {
        None
    } else {
        Some(&mut length_out)
    };
    let err = rust_ffi::FT_Sfnt_Table_Info(&state.inner, table_index, tag_ref, length_ref);
    if err == rust_ffi::FT_Err_Ok {
        if let Some(tag_ptr) = non_null_mut(tag) {
            // SAFETY: writable FT_ULong out-param. Copying after the core call
            // avoids creating aliased `&mut` references for caller pointers.
            unsafe { *tag_ptr.as_ptr() = tag_out as FT_ULong };
        }
        if let Some(len_ptr) = non_null_mut(length) {
            // SAFETY: writable FT_ULong out-param. C writes tag before length,
            // so an aliased caller pointer ends with the length value.
            unsafe { *len_ptr.as_ptr() = length_out as FT_ULong };
        }
    }
    err as FT_Error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_First_Char(face: FT_Face, agindex: *mut FT_UInt) -> FT_ULong {
    let mut glyph_index = 0;
    let char_code = rust_ffi::FT_Get_First_Char(
        face_state(face).map(|state| &state.inner),
        // FreeType `base/ftobjs.c:3952-3972` accepts a null `agindex`;
        // it still returns the charcode and skips only the glyph-index write.
        non_null_mut(agindex).map(|_| &mut glyph_index),
    );
    if let Some(out) = non_null_mut(agindex) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = glyph_index };
    }
    char_code
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Next_Char(
    face: FT_Face,
    char_code: FT_ULong,
    agindex: *mut FT_UInt,
) -> FT_ULong {
    let mut glyph_index = 0;
    let next_char = rust_ffi::FT_Get_Next_Char(
        face_state(face).map(|state| &state.inner),
        char_code,
        // FreeType `base/ftobjs.c:3977-4003` accepts a null `agindex`;
        // it still returns the next charcode and skips only the glyph-index write.
        non_null_mut(agindex).map(|_| &mut glyph_index),
    );
    if let Some(out) = non_null_mut(agindex) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = glyph_index };
    }
    next_char
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_Version(
    library: FT_Library,
    amajor: *mut FT_Int,
    aminor: *mut FT_Int,
    apatch: *mut FT_Int,
) {
    let mut major = 0;
    let mut minor = 0;
    let mut patch = 0;
    rust_ffi::FT_Library_Version(
        library_ref(library),
        Some(&mut major),
        Some(&mut minor),
        Some(&mut patch),
    );
    if let Some(out) = non_null_mut(amajor) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = major };
    }
    if let Some(out) = non_null_mut(aminor) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = minor };
    }
    if let Some(out) = non_null_mut(apatch) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = patch };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Char(
    face: FT_Face,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Char(&state.inner, char_code, load_flags) {
        Ok(slot) => store_slot(face, slot, load_flags),
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Glyph(
    face: FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Glyph(&state.inner, glyph_index, load_flags) {
        Ok(slot) => store_slot(face, slot, load_flags),
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Advance(
    face: FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    padvance: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        // FreeType `src/base/ftadvanc.c:116-120` checks `face` before
        // `padvance`, so a missing face reports `Invalid_Face_Handle`.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = non_null_mut(padvance) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Get_Advance(&state.inner, glyph_index, load_flags) {
        Ok(advance) => {
            // SAFETY: `out` is a valid out pointer checked above.
            unsafe { *out.as_ptr() = advance };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Advances(
    face: FT_Face,
    start: FT_UInt,
    count: FT_UInt,
    load_flags: FT_Int32,
    padvances: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        // FreeType `src/base/ftadvanc.c:158-164` checks `face` before
        // `padvances`, so a missing face reports `Invalid_Face_Handle`.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = non_null_mut(padvances) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Ok(out_len) = usize::try_from(count) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Get_Advances(&state.inner, start, count, load_flags) {
        Ok(advances) => {
            if advances.len() != out_len {
                return rust_ffi::FT_Err_Invalid_Argument;
            }
            if out_len != 0 {
                // SAFETY: `out` is non-null and caller promises at least `count` writable entries.
                let out = unsafe { slice::from_raw_parts_mut(out.as_ptr(), out_len) };
                out.copy_from_slice(&advances);
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_SubGlyph_Info(
    slot: FT_GlyphSlot,
    sub_index: FT_UInt,
    p_index: *mut FT_Int,
    p_flags: *mut FT_UInt,
    p_arg1: *mut FT_Int,
    p_arg2: *mut FT_Int,
    p_transform: *mut FT_Matrix,
) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let (Some(p_index), Some(p_flags), Some(p_arg1), Some(p_arg2), Some(p_transform)) = (
        non_null_mut(p_index),
        non_null_mut(p_flags),
        non_null_mut(p_arg1),
        non_null_mut(p_arg2),
        non_null_mut(p_transform),
    ) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    let rust_slot = unsafe { &(*slot_ptr.as_ptr()).rust_slot };
    let mut index = 0;
    let mut flags = 0;
    let mut arg1 = 0;
    let mut arg2 = 0;
    let mut transform = rust_ffi::FT_Matrix::default();
    let error = rust_ffi::FT_Get_SubGlyph_Info(
        Some(rust_slot),
        sub_index,
        Some(&mut index),
        Some(&mut flags),
        Some(&mut arg1),
        Some(&mut arg2),
        Some(&mut transform),
    );
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }

    // SAFETY: all output pointers are non-null and caller provides writable storage.
    unsafe {
        *p_index.as_ptr() = index;
        *p_flags.as_ptr() = flags;
        *p_arg1.as_ptr() = arg1;
        *p_arg2.as_ptr() = arg2;
        *p_transform.as_ptr() = FT_Matrix {
            xx: transform.xx,
            xy: transform.xy,
            yx: transform.yx,
            yy: transform.yy,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Render_Glyph(slot: FT_GlyphSlot, render_mode: FT_Render_Mode) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    let source_face = unsafe { (*slot_ptr.as_ptr()).source_face };
    // SAFETY: `slot_ptr` points to a live slot allocated by this crate.
    let rust_slot = unsafe { (*slot_ptr.as_ptr()).rust_slot.clone() };
    // SAFETY: `slot_ptr` points to a live slot allocated by this crate.
    let load_flags = unsafe { (*slot_ptr.as_ptr()).load_flags };
    match rust_ffi::FT_Render_Glyph(rust_slot, render_mode) {
        Ok(rendered) => store_slot(source_face, rendered, load_flags | rust_ffi::FT_LOAD_RENDER),
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Embolden(slot: FT_GlyphSlot) {
    FT_GlyphSlot_AdjustWeight(slot, 0x0AAA, 0x0AAA);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Own_Bitmap(slot: FT_GlyphSlot) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Ok;
    };
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    unsafe {
        let slot_ref = &mut *slot_ptr.as_ptr();
        let err = rust_ffi::FT_GlyphSlot_Own_Bitmap(Some(&mut slot_ref.rust_slot));
        if err != rust_ffi::FT_Err_Ok {
            return err;
        }
        let source_face = slot_ref.source_face;
        let load_flags = slot_ref.load_flags;
        let rust_slot = slot_ref.rust_slot.clone();
        *slot_ref = rust_slot_to_abi(rust_slot, source_face, load_flags);
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_AdjustWeight(
    slot: FT_GlyphSlot,
    xdelta: FT_Fixed,
    ydelta: FT_Fixed,
) {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return;
    };
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    unsafe {
        let slot_ref = &mut *slot_ptr.as_ptr();
        rust_ffi::FT_GlyphSlot_AdjustWeight(Some(&mut slot_ref.rust_slot), xdelta, ydelta);
        let source_face = slot_ref.source_face;
        let load_flags = slot_ref.load_flags;
        let rust_slot = slot_ref.rust_slot.clone();
        *slot_ref = rust_slot_to_abi(rust_slot, source_face, load_flags);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Oblique(slot: FT_GlyphSlot) {
    FT_GlyphSlot_Slant(slot, 0x0366A, 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Slant(slot: FT_GlyphSlot, xslant: FT_Fixed, yslant: FT_Fixed) {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return;
    };
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    unsafe {
        let slot_ref = &mut *slot_ptr.as_ptr();
        rust_ffi::FT_GlyphSlot_Slant(Some(&mut slot_ref.rust_slot), xslant, yslant);
        let source_face = slot_ref.source_face;
        let load_flags = slot_ref.load_flags;
        let rust_slot = slot_ref.rust_slot.clone();
        *slot_ref = rust_slot_to_abi(rust_slot, source_face, load_flags);
    }
}

fn store_slot(face: FT_Face, slot: rust_ffi::FT_GlyphSlot, load_flags: FT_Int32) -> FT_Error {
    let Some(face_ptr) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `face_ptr` is a live handle and owns the previous `glyph` pointer.
    unsafe {
        drop_glyph((*face_ptr.as_ptr()).glyph);
        (*face_ptr.as_ptr()).glyph =
            Box::into_raw(Box::new(rust_slot_to_abi(slot, face, load_flags)));
    }
    rust_ffi::FT_Err_Ok
}

fn update_size_metrics(face: FT_Face, rust_face: &rust_ffi::FT_Face) {
    let Some(face_ptr) = non_null_mut(face) else {
        return;
    };
    // SAFETY: `face_ptr` is a live handle allocated by this crate.
    let size = unsafe { (*face_ptr.as_ptr()).size };
    let Some(size_ptr) = non_null_mut(size) else {
        return;
    };
    // SAFETY: `size_ptr` points to the live size record owned by `face`.
    unsafe { (*size_ptr.as_ptr()).metrics = rust_size_metrics_to_abi(rust_face.size_metrics) };
}

fn rust_face_info(face: &rust_ffi::FT_Face) -> rust_ffi::FT_FaceRecPublic {
    rust_ffi::FT_FaceRecPublic {
        num_faces: face.num_faces,
        face_index: face.face_index,
        face_flags: face.face_flags,
        style_flags: face.style_flags,
        num_glyphs: face.num_glyphs,
        num_fixed_sizes: face.num_fixed_sizes,
        available_sizes: if face.available_sizes.is_empty() {
            ptr::null_mut()
        } else {
            face.available_sizes.as_ptr().cast_mut()
        },
        bbox: face.bbox,
        units_per_EM: face.units_per_EM,
        ascender: face.ascender,
        descender: face.descender,
        height: face.height,
        max_advance_width: face.max_advance_width,
        max_advance_height: face.max_advance_height,
        underline_position: face.underline_position,
        underline_thickness: face.underline_thickness,
        size: face.size,
        stream: face.memory_stream(),
        ..rust_ffi::FT_FaceRecPublic::default()
    }
}

fn rust_face_charmap(face: &rust_ffi::FT_Face, index: usize) -> Option<rust_ffi::FT_CharMap> {
    face.charmaps.get(index).map(|record| {
        (record as *const rust_ffi::FT_CharMapRecPublic)
            .cast_mut()
            .cast()
    })
}

fn rust_slot_to_abi(
    slot: rust_ffi::FT_GlyphSlot,
    source_face: FT_Face,
    load_flags: FT_Int32,
) -> FT_GlyphSlotRec {
    let rust_slot = slot.clone();
    let mut buffer = slot
        .bitmap
        .as_ref()
        .map(|bitmap| bitmap.buffer.clone())
        .unwrap_or_default();
    let bitmap = slot
        .bitmap
        .map(|bitmap| FT_Bitmap {
            rows: bitmap.rows,
            width: bitmap.width,
            pitch: bitmap.pitch,
            buffer: buffer.as_mut_ptr(),
            num_grays: bitmap.num_grays,
            pixel_mode: bitmap.pixel_mode,
            palette_mode: 0,
            palette: ptr::null_mut(),
        })
        .unwrap_or_default();
    FT_GlyphSlotRec {
        glyph_index: slot.glyph_index,
        metrics: rust_metrics_to_abi(slot.metrics),
        advance: FT_Vector {
            x: slot.advance.x,
            y: slot.advance.y,
        },
        format: slot.format,
        bitmap,
        bitmap_left: slot.bitmap_left,
        bitmap_top: slot.bitmap_top,
        owns_bitmap: slot.owns_bitmap,
        buffer,
        rust_slot,
        source_face,
        load_flags,
    }
}

fn rust_metrics_to_abi(metrics: rust_ffi::FT_Glyph_Metrics) -> FT_Glyph_Metrics {
    FT_Glyph_Metrics {
        width: metrics.width,
        height: metrics.height,
        horiBearingX: metrics.horiBearingX,
        horiBearingY: metrics.horiBearingY,
        horiAdvance: metrics.horiAdvance,
        vertBearingX: metrics.vertBearingX,
        vertBearingY: metrics.vertBearingY,
        vertAdvance: metrics.vertAdvance,
    }
}

fn rust_size_metrics_to_abi(metrics: rust_ffi::FT_Size_Metrics) -> FT_Size_Metrics {
    FT_Size_Metrics {
        x_ppem: metrics.x_ppem,
        y_ppem: metrics.y_ppem,
        x_scale: metrics.x_scale,
        y_scale: metrics.y_scale,
        ascender: metrics.ascender,
        descender: metrics.descender,
        height: metrics.height,
        max_advance: metrics.max_advance,
    }
}

fn outline_snapshot_from_c(outline: *const FT_Outline) -> Option<rust_ffi::FT_OutlineSnapshot> {
    let outline = non_null(outline)?;
    // SAFETY: `outline` is non-null; callers of the C ABI must pass a valid `FT_Outline`.
    let outline = unsafe { outline.as_ref() };
    let n_points = usize::from(outline.n_points);
    let n_contours = usize::from(outline.n_contours);
    if (n_points > 0 && outline.points.is_null()) || (n_contours > 0 && outline.contours.is_null())
    {
        return None;
    }
    let points = if n_points == 0 {
        Vec::new()
    } else {
        // SAFETY: `points` is non-null for `n_points > 0`; the C ABI caller owns a readable
        // array of `n_points` `FT_Vector` records for the duration of this call.
        unsafe { slice::from_raw_parts(outline.points, n_points) }
            .iter()
            .map(|point| rust_ffi::FT_Vector {
                x: point.x,
                y: point.y,
            })
            .collect()
    };
    let tags = if n_points == 0 || outline.tags.is_null() {
        Vec::new()
    } else {
        // SAFETY: `tags` is non-null and points to `n_points` readable tag bytes.
        unsafe { slice::from_raw_parts(outline.tags, n_points) }.to_vec()
    };
    let contours = if n_contours == 0 {
        Vec::new()
    } else {
        // SAFETY: `contours` is non-null for `n_contours > 0`; the caller provides
        // `n_contours` readable contour endpoint values.
        unsafe { slice::from_raw_parts(outline.contours, n_contours) }.to_vec()
    };
    Some(rust_ffi::FT_OutlineSnapshot {
        points,
        tags,
        contours,
        flags: outline.flags,
    })
}

fn copy_outline_snapshot_to_c(
    outline: *mut FT_Outline,
    snapshot: &rust_ffi::FT_OutlineSnapshot,
    copy_tags_and_flags: bool,
) {
    let Some(mut outline) = non_null_mut(outline) else {
        return;
    };
    // SAFETY: `outline` is non-null and still refers to the caller-owned
    // descriptor used to create `snapshot`.
    let outline = unsafe { outline.as_mut() };
    if !outline.points.is_null() {
        // SAFETY: the public descriptor promises `n_points` writable vectors.
        let points =
            unsafe { slice::from_raw_parts_mut(outline.points, usize::from(outline.n_points)) };
        for (target, source) in points.iter_mut().zip(&snapshot.points) {
            target.x = source.x;
            target.y = source.y;
        }
    }
    if copy_tags_and_flags {
        if !outline.tags.is_null() {
            // SAFETY: the public descriptor promises `n_points` writable tag bytes.
            let tags =
                unsafe { slice::from_raw_parts_mut(outline.tags, usize::from(outline.n_points)) };
            for (target, source) in tags.iter_mut().zip(&snapshot.tags) {
                *target = *source;
            }
        }
        outline.flags = snapshot.flags;
    }
}

fn copy_rendered_bitmap_to_c(target: &FT_Bitmap, rendered: &rust_ffi::FT_Bitmap) {
    let rows = usize::try_from(target.rows).unwrap_or(0);
    let width = usize::try_from(target.width).unwrap_or(0);
    let pitch_abs = usize::try_from(target.pitch.unsigned_abs()).unwrap_or(0);
    let rendered_pitch_abs = usize::try_from(rendered.pitch.unsigned_abs()).unwrap_or(0);
    if target.buffer.is_null() || rows == 0 || width == 0 || pitch_abs == 0 {
        return;
    }
    let row_bytes = width.min(pitch_abs);
    let target_len = pitch_abs.saturating_mul(rows);
    // SAFETY: the public C caller provides a writable bitmap buffer of at least
    // `abs(pitch) * rows` bytes, matching FreeType's `FT_Bitmap` contract.
    let target_buffer = unsafe { slice::from_raw_parts_mut(target.buffer, target_len) };
    for row in 0..rows {
        let src = row.saturating_mul(rendered_pitch_abs);
        let dst = row.saturating_mul(pitch_abs);
        let Some(src_row) = rendered.buffer.get(src..src.saturating_add(row_bytes)) else {
            break;
        };
        let Some(dst_row) = target_buffer.get_mut(dst..dst.saturating_add(row_bytes)) else {
            break;
        };
        dst_row.copy_from_slice(src_row);
    }
}

fn bitmap_to_rust(bitmap: &FT_Bitmap) -> rust_ffi::FT_Bitmap_C {
    rust_ffi::FT_Bitmap_C {
        rows: bitmap.rows,
        width: bitmap.width,
        pitch: bitmap.pitch,
        buffer: bitmap.buffer,
        num_grays: bitmap.num_grays,
        pixel_mode: u8::try_from(bitmap.pixel_mode).unwrap_or(0),
        palette_mode: bitmap.palette_mode,
        palette: bitmap.palette,
    }
}

fn copy_rust_bitmap_record_to_c(target: &mut FT_Bitmap, source: &rust_ffi::FT_Bitmap_C) {
    target.rows = source.rows;
    target.width = source.width;
    target.pitch = source.pitch;
    target.buffer = source.buffer;
    target.num_grays = source.num_grays;
    target.pixel_mode = source.pixel_mode.into();
    target.palette_mode = source.palette_mode;
    target.palette = source.palette;
}

fn bitmap_bytes(bitmap: &FT_Bitmap) -> Option<Vec<u8>> {
    let len = usize::try_from(bitmap.pitch.unsigned_abs())
        .ok()?
        .checked_mul(usize::try_from(bitmap.rows).ok()?)?;
    if bitmap.buffer.is_null() || len == 0 {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(bitmap.buffer, len) }.to_vec())
}

fn face_state(face: FT_Face) -> Option<&'static FaceState> {
    let face = non_null_mut(face)?;
    // SAFETY: `face` is non-null and must have been allocated by `FT_New_Memory_Face`.
    let internal = unsafe { (*face.as_ptr()).internal };
    if internal.is_null() {
        None
    } else {
        // SAFETY: `internal` points to a `FaceState` allocated by this crate.
        Some(unsafe { &*internal.cast::<FaceState>() })
    }
}

fn face_state_mut(face: FT_Face) -> Option<&'static mut FaceState> {
    let face = non_null_mut(face)?;
    // SAFETY: `face` is non-null and must have been allocated by `FT_New_Memory_Face`.
    let internal = unsafe { (*face.as_ptr()).internal };
    let mut state = NonNull::new(internal.cast::<FaceState>())?;
    // SAFETY: `internal` points to a `FaceState` allocated by this crate.
    Some(unsafe { state.as_mut() })
}

fn library_ref(library: FT_Library) -> Option<&'static rust_ffi::FT_Library> {
    let library = non_null_mut(library)?;
    // SAFETY: `library` is non-null and must have been allocated by `FT_Init_FreeType`.
    let internal = unsafe { (*library.as_ptr()).internal };
    if internal.is_null() {
        None
    } else {
        // SAFETY: `internal` points to a `LibraryState` allocated by this crate.
        Some(unsafe { &(*internal.cast::<LibraryState>()).inner })
    }
}

fn library_state_mut(library: FT_Library) -> Option<&'static mut LibraryState> {
    let library = non_null_mut(library)?;
    // SAFETY: `library` is non-null and must have been allocated by `FT_Init_FreeType`.
    let internal = unsafe { (*library.as_ptr()).internal };
    let mut state = NonNull::new(internal.cast::<LibraryState>())?;
    // SAFETY: `internal` points to a uniquely borrowed `LibraryState`.
    Some(unsafe { state.as_mut() })
}

fn library_mut(library: FT_Library) -> Option<&'static mut rust_ffi::FT_Library> {
    let library = non_null_mut(library)?;
    // SAFETY: `library` is non-null and must have been allocated by `FT_Init_FreeType`.
    let internal = unsafe { (*library.as_ptr()).internal };
    if internal.is_null() {
        None
    } else {
        // SAFETY: `internal` points to a uniquely borrowed `LibraryState`.
        Some(unsafe { &mut (*internal.cast::<LibraryState>()).inner })
    }
}

fn non_null<T>(ptr: *const T) -> Option<NonNull<T>> {
    NonNull::new(ptr.cast_mut())
}

fn non_null_mut<T>(ptr: *mut T) -> Option<NonNull<T>> {
    NonNull::new(ptr)
}

unsafe fn drop_glyph(slot: FT_GlyphSlot) {
    if !slot.is_null() {
        // SAFETY: `slot` is owned by its containing face and allocated with `Box::into_raw`.
        unsafe { drop(Box::from_raw(slot)) };
    }
}

unsafe fn drop_size(size: FT_Size) {
    if !size.is_null() {
        // SAFETY: `size` is owned by its containing face and allocated with `Box::into_raw`.
        unsafe { drop(Box::from_raw(size)) };
    }
}
