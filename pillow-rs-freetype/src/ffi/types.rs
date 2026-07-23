#![allow(non_camel_case_types, non_snake_case)]

use std::ffi::c_void;
use std::os::raw::{c_char, c_int, c_long, c_schar, c_short, c_uchar, c_uint, c_ushort};

pub type FT_Error = c_int;
pub type FT_Int = c_int;
pub type FT_UInt = c_uint;
pub type FT_Int32 = i32;
pub type FT_UInt32 = u32;
pub type FT_Byte = c_uchar;
pub type FT_Bool = c_uchar;
pub type FT_Char = c_schar;
pub type FT_Short = c_short;
#[cfg(all(target_arch = "wasm32", feature = "wasm-wide-internals"))]
pub type FT_Long = i64;
#[cfg(not(all(target_arch = "wasm32", feature = "wasm-wide-internals")))]
pub type FT_Long = c_long;
#[cfg(all(target_arch = "wasm32", feature = "wasm-wide-internals"))]
pub type FT_ULong = u64;
#[cfg(not(all(target_arch = "wasm32", feature = "wasm-wide-internals")))]
pub type FT_ULong = std::os::raw::c_ulong;
pub type FT_Pos = FT_Long;
pub type FT_Fixed = FT_Long;
pub type FT_Angle = FT_Fixed;
pub type FT_F26Dot6 = FT_Long;
pub type FT_UShort = c_ushort;
pub type FT_FWord = c_short;
pub type FT_UFWord = c_ushort;
pub type FT_F2Dot14 = c_short;
pub type FT_UInt16 = c_ushort;
pub type FT_Tag = u32;
pub type FT_Offset = usize;
pub type FT_PtrDist = isize;
pub type FT_String = c_char;
pub type FT_Pointer = *mut c_void;
pub type FT_Bytes = *const FT_Byte;
pub type FT_Generic_Finalizer = FT_Pointer;
pub type FT_Render_Mode = c_int;
pub type FT_Pixel_Mode = c_int;
pub type FT_Glyph_Format = c_int;
pub type FT_Orientation = c_int;
pub type FT_Sfnt_Tag = c_uint;
pub type FT_Encoding = c_int;
pub type FT_Size_Request_Type = c_int;
pub type FT_LcdFilter = c_int;
pub type FT_TrueTypeEngineType = c_int;
pub type PS_Dict_Keys = c_int;
pub type T1_EncodingType = c_int;
pub type FT_PaintExtend = FT_Int;
pub type FT_Composite_Mode = FT_Int;
pub type FT_Incremental = FT_Pointer;
pub type FT_Incremental_Metrics = *mut FT_Incremental_MetricsRec;
pub type FT_Incremental_Interface = *mut FT_Incremental_InterfaceRec;
pub type FT_Module_Interface = FT_Pointer;
pub type FT_Outline_MoveToFunc =
    Option<unsafe extern "C" fn(to: *const FT_Vector, user: FT_Pointer) -> c_int>;
pub type FT_Outline_MoveTo_Func = FT_Outline_MoveToFunc;
pub type FT_SpanFunc =
    Option<unsafe extern "C" fn(y: c_int, count: c_int, spans: *const FT_Span, user: FT_Pointer)>;
pub type FT_Raster_Span_Func = FT_SpanFunc;
pub type FT_Raster_NewFunc =
    Option<unsafe extern "C" fn(memory: FT_Pointer, raster: *mut FT_Raster) -> c_int>;
pub type FT_Raster_New_Func = FT_Raster_NewFunc;
pub type FT_Raster_DoneFunc = Option<unsafe extern "C" fn(raster: FT_Raster)>;
pub type FT_Raster_Done_Func = FT_Raster_DoneFunc;
pub type FT_Raster_ResetFunc =
    Option<unsafe extern "C" fn(raster: FT_Raster, pool_base: *mut FT_Byte, pool_size: FT_ULong)>;
pub type FT_Raster_Reset_Func = FT_Raster_ResetFunc;
pub type FT_Raster_SetModeFunc =
    Option<unsafe extern "C" fn(raster: FT_Raster, mode: FT_ULong, args: FT_Pointer) -> c_int>;
pub type FT_Raster_Set_Mode_Func = FT_Raster_SetModeFunc;
pub type FT_Raster_RenderFunc =
    Option<unsafe extern "C" fn(raster: FT_Raster, params: *const FT_Raster_Params) -> c_int>;
pub type FT_Raster_Render_Func = FT_Raster_RenderFunc;
pub type FT_DebugHook_Func = Option<extern "C" fn(arg: FT_Pointer) -> FT_Error>;
pub type FT_Glyph_InitFunc =
    Option<unsafe extern "C" fn(glyph: FT_Glyph, slot: FT_Pointer) -> FT_Error>;
pub type FT_Glyph_Init_Func = FT_Glyph_InitFunc;
pub type FT_Glyph_DoneFunc = Option<unsafe extern "C" fn(glyph: FT_Glyph)>;
pub type FT_Glyph_Done_Func = FT_Glyph_DoneFunc;
pub type FT_Glyph_CopyFunc =
    Option<unsafe extern "C" fn(source: FT_Glyph, target: FT_Glyph) -> FT_Error>;
pub type FT_Glyph_Copy_Func = FT_Glyph_CopyFunc;
pub type FT_Glyph_TransformFunc = Option<
    unsafe extern "C" fn(glyph: FT_Glyph, matrix: *const FT_Matrix, delta: *const FT_Vector),
>;
pub type FT_Glyph_Transform_Func = FT_Glyph_TransformFunc;
pub type FT_Glyph_GetBBoxFunc = Option<unsafe extern "C" fn(glyph: FT_Glyph, abbox: *mut FT_BBox)>;
pub type FT_Glyph_BBox_Func = FT_Glyph_GetBBoxFunc;
pub type FT_Glyph_PrepareFunc =
    Option<unsafe extern "C" fn(glyph: FT_Glyph, slot: FT_Pointer) -> FT_Error>;
pub type FT_Glyph_Prepare_Func = FT_Glyph_PrepareFunc;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_UnitVector {
    pub x: FT_F2Dot14,
    pub y: FT_F2Dot14,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Matrix {
    pub xx: FT_Fixed,
    pub xy: FT_Fixed,
    pub yx: FT_Fixed,
    pub yy: FT_Fixed,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Data {
    pub pointer: FT_Bytes,
    pub length: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FT_Generic {
    pub data: FT_Pointer,
    pub finalizer: FT_Generic_Finalizer,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_ListNodeRec {
    pub prev: FT_ListNode,
    pub next: FT_ListNode,
    pub data: FT_Pointer,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_ListRec {
    pub head: FT_ListNode,
    pub tail: FT_ListNode,
}

#[repr(C)]
pub struct FT_SizeRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FT_RendererRec {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union FT_StreamDesc {
    pub value: c_long,
    pub pointer: FT_Pointer,
}

impl Default for FT_StreamDesc {
    fn default() -> Self {
        Self {
            pointer: std::ptr::null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_StreamRec {
    pub base: *mut FT_Byte,
    pub size: FT_ULong,
    pub pos: FT_ULong,
    pub descriptor: FT_StreamDesc,
    pub pathname: FT_StreamDesc,
    pub read: FT_Pointer,
    pub close: FT_Pointer,
    pub memory: FT_Memory,
    pub cursor: *mut FT_Byte,
    pub limit: *mut FT_Byte,
}

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

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_MemoryRec {
    pub user: FT_Pointer,
    pub alloc: FT_Alloc_Func,
    pub free: FT_Free_Func,
    pub realloc: FT_Realloc_Func,
}

#[repr(C)]
pub struct FT_RasterRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FT_ModuleRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FT_DriverRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FT_CharMapRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FT_SubGlyphRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FT_Size_InternalRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FT_Slot_InternalRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FT_Face_InternalRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FTC_ManagerRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FTC_NodeRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FTC_CMapCacheRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FTC_ImageCacheRec {
    _private: [u8; 0],
}

#[repr(C)]
pub struct FTC_SBitCacheRec {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FTC_ScalerRec {
    pub face_id: FTC_FaceID,
    pub width: FT_UInt,
    pub height: FT_UInt,
    pub pixel: FT_Int,
    pub x_res: FT_UInt,
    pub y_res: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FTC_ImageTypeRec {
    pub face_id: FTC_FaceID,
    pub width: FT_UInt,
    pub height: FT_UInt,
    pub flags: FT_Int32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FTC_SBitRec {
    pub width: FT_Byte,
    pub height: FT_Byte,
    pub left: FT_Char,
    pub top: FT_Char,
    pub format: FT_Byte,
    pub max_grays: FT_Byte,
    pub pitch: FT_Short,
    pub xadvance: FT_Char,
    pub yadvance: FT_Char,
    pub buffer: *mut FT_Byte,
}

pub type FT_ListNode = *mut FT_ListNodeRec;
pub type FT_List = *mut FT_ListRec;
pub type FT_Size = *mut FT_SizeRec;
pub type FT_Renderer = *mut FT_RendererRec;
pub type FT_Stream = *mut FT_StreamRec;
pub type FT_Memory = *mut FT_MemoryRec;
pub type FT_Raster = *mut FT_RasterRec;
pub type FT_Module = *mut FT_ModuleRec;
pub type FT_Driver = *mut FT_DriverRec;
pub type FT_CharMap = *mut FT_CharMapRec;
pub type FT_SubGlyph = *mut FT_SubGlyphRec;
pub type FT_Size_Internal = *mut FT_Size_InternalRec;
pub type FT_Slot_Internal = *mut FT_Slot_InternalRec;
pub type FT_Face_Internal = *mut FT_Face_InternalRec;
pub type FT_Glyph = *mut FT_GlyphRec;
pub type FT_BitmapGlyph = *mut FT_BitmapGlyphRec;
pub type FT_OutlineGlyph = *mut FT_OutlineGlyphRec;
pub type FT_SvgGlyph = *mut FT_SvgGlyphRec;

pub type FTC_FaceID = FT_Pointer;
pub type FTC_Manager = *mut FTC_ManagerRec;
pub type FTC_Node = *mut FTC_NodeRec;
pub type FTC_CMapCache = *mut FTC_CMapCacheRec;
pub type FTC_ImageCache = *mut FTC_ImageCacheRec;
pub type FTC_SBitCache = *mut FTC_SBitCacheRec;
pub type FTC_Scaler = *mut FTC_ScalerRec;
pub type FTC_ImageType = *mut FTC_ImageTypeRec;
pub type FTC_SBit = *mut FTC_SBitRec;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Bitmap_Size {
    pub height: FT_Short,
    pub width: FT_Short,
    pub size: FT_Pos,
    pub x_ppem: FT_Pos,
    pub y_ppem: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_CharMapRecPublic {
    pub face: FT_Pointer,
    pub encoding: FT_Encoding,
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Parameter {
    pub tag: FT_ULong,
    pub data: FT_Pointer,
}

pub type BDF_PropertyType = i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub union BDF_PropertyValue {
    pub atom: *const FT_String,
    pub integer: FT_Int32,
    pub cardinal: FT_UInt32,
}

impl Default for BDF_PropertyValue {
    fn default() -> Self {
        Self {
            atom: std::ptr::null(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BDF_PropertyRec {
    pub type_: BDF_PropertyType,
    pub u: BDF_PropertyValue,
}

pub type BDF_Property = *mut BDF_PropertyRec;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Open_Args {
    pub flags: FT_UInt,
    pub memory_base: *const FT_Byte,
    pub memory_size: FT_Long,
    pub pathname: *mut FT_String,
    pub stream: FT_Stream,
    pub driver: FT_Module,
    pub num_params: FT_Int,
    pub params: *mut FT_Parameter,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Size_RequestRec {
    pub type_: FT_Size_Request_Type,
    pub width: FT_Long,
    pub height: FT_Long,
    pub horiResolution: FT_UInt,
    pub vertResolution: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FT_WinFNT_HeaderRec {
    pub version: FT_UShort,
    pub file_size: FT_ULong,
    pub copyright: [FT_Byte; 60],
    pub file_type: FT_UShort,
    pub nominal_point_size: FT_UShort,
    pub vertical_resolution: FT_UShort,
    pub horizontal_resolution: FT_UShort,
    pub ascent: FT_UShort,
    pub internal_leading: FT_UShort,
    pub external_leading: FT_UShort,
    pub italic: FT_Byte,
    pub underline: FT_Byte,
    pub strike_out: FT_Byte,
    pub weight: FT_UShort,
    pub charset: FT_Byte,
    pub pixel_width: FT_UShort,
    pub pixel_height: FT_UShort,
    pub pitch_and_family: FT_Byte,
    pub avg_width: FT_UShort,
    pub max_width: FT_UShort,
    pub first_char: FT_Byte,
    pub last_char: FT_Byte,
    pub default_char: FT_Byte,
    pub break_char: FT_Byte,
    pub bytes_per_row: FT_UShort,
    pub device_offset: FT_ULong,
    pub face_name_offset: FT_ULong,
    pub bits_pointer: FT_ULong,
    pub bits_offset: FT_ULong,
    pub reserved: FT_Byte,
    pub flags: FT_ULong,
    pub A_space: FT_UShort,
    pub B_space: FT_UShort,
    pub C_space: FT_UShort,
    pub color_table_offset: FT_UShort,
    pub reserved1: [FT_ULong; 4],
}

pub type FT_WinFNT_Header = *mut FT_WinFNT_HeaderRec;

impl Default for FT_WinFNT_HeaderRec {
    fn default() -> Self {
        Self {
            version: 0,
            file_size: 0,
            copyright: [0; 60],
            file_type: 0,
            nominal_point_size: 0,
            vertical_resolution: 0,
            horizontal_resolution: 0,
            ascent: 0,
            internal_leading: 0,
            external_leading: 0,
            italic: 0,
            underline: 0,
            strike_out: 0,
            weight: 0,
            charset: 0,
            pixel_width: 0,
            pixel_height: 0,
            pitch_and_family: 0,
            avg_width: 0,
            max_width: 0,
            first_char: 0,
            last_char: 0,
            default_char: 0,
            break_char: 0,
            bytes_per_row: 0,
            device_offset: 0,
            face_name_offset: 0,
            bits_pointer: 0,
            bits_offset: 0,
            reserved: 0,
            flags: 0,
            A_space: 0,
            B_space: 0,
            C_space: 0,
            color_table_offset: 0,
            reserved1: [0; 4],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FT_SizeRecPublic {
    pub face: FT_Pointer,
    pub generic: FT_Generic,
    pub metrics: FT_Size_Metrics,
    pub internal: FT_Size_Internal,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FT_FaceRecPublic {
    pub num_faces: FT_Long,
    pub face_index: FT_Long,
    pub face_flags: FT_Long,
    pub style_flags: FT_Long,
    pub num_glyphs: FT_Long,
    pub family_name: *mut FT_String,
    pub style_name: *mut FT_String,
    pub num_fixed_sizes: FT_Int,
    pub available_sizes: *mut FT_Bitmap_Size,
    pub num_charmaps: FT_Int,
    pub charmaps: *mut FT_CharMap,
    pub generic: FT_Generic,
    pub bbox: FT_BBox,
    pub units_per_EM: FT_UShort,
    pub ascender: FT_Short,
    pub descender: FT_Short,
    pub height: FT_Short,
    pub max_advance_width: FT_Short,
    pub max_advance_height: FT_Short,
    pub underline_position: FT_Short,
    pub underline_thickness: FT_Short,
    pub glyph: FT_Pointer,
    pub size: FT_Size,
    pub charmap: FT_CharMap,
    pub driver: FT_Driver,
    pub memory: FT_Memory,
    pub stream: FT_Stream,
    pub sizes_list: FT_ListRec,
    pub autohint: FT_Generic,
    pub extensions: FT_Pointer,
    pub internal: FT_Face_Internal,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PS_FontInfoRec {
    pub version: *mut FT_String,
    pub notice: *mut FT_String,
    pub full_name: *mut FT_String,
    pub family_name: *mut FT_String,
    pub weight: *mut FT_String,
    pub italic_angle: FT_Fixed,
    pub is_fixed_pitch: FT_Bool,
    pub underline_position: FT_Short,
    pub underline_thickness: FT_UShort,
}

pub type PS_FontInfo = *mut PS_FontInfoRec;
pub type T1_FontInfo = PS_FontInfoRec;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PS_PrivateRec {
    pub unique_id: FT_Int,
    pub lenIV: FT_Int,
    pub num_blue_values: FT_Byte,
    pub num_other_blues: FT_Byte,
    pub num_family_blues: FT_Byte,
    pub num_family_other_blues: FT_Byte,
    pub blue_values: [FT_Short; 14],
    pub other_blues: [FT_Short; 10],
    pub family_blues: [FT_Short; 14],
    pub family_other_blues: [FT_Short; 10],
    pub blue_scale: FT_Fixed,
    pub blue_shift: FT_Int,
    pub blue_fuzz: FT_Int,
    pub standard_width: [FT_UShort; 1],
    pub standard_height: [FT_UShort; 1],
    pub num_snap_widths: FT_Byte,
    pub num_snap_heights: FT_Byte,
    pub force_bold: FT_Bool,
    pub round_stem_up: FT_Bool,
    pub snap_widths: [FT_Short; 13],
    pub snap_heights: [FT_Short; 13],
    pub expansion_factor: FT_Fixed,
    pub language_group: FT_Long,
    pub password: FT_Long,
    pub min_feature: [FT_Short; 2],
}

pub type PS_Private = *mut PS_PrivateRec;
pub type T1_Private = PS_PrivateRec;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TT_Header {
    pub Table_Version: FT_Fixed,
    pub Font_Revision: FT_Fixed,
    pub CheckSum_Adjust: FT_Long,
    pub Magic_Number: FT_Long,
    pub Flags: FT_UShort,
    pub Units_Per_EM: FT_UShort,
    pub Created: [FT_ULong; 2],
    pub Modified: [FT_ULong; 2],
    pub xMin: FT_Short,
    pub yMin: FT_Short,
    pub xMax: FT_Short,
    pub yMax: FT_Short,
    pub Mac_Style: FT_UShort,
    pub Lowest_Rec_PPEM: FT_UShort,
    pub Font_Direction: FT_Short,
    pub Index_To_Loc_Format: FT_Short,
    pub Glyph_Data_Format: FT_Short,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TT_HoriHeader {
    pub Version: FT_Fixed,
    pub Ascender: FT_Short,
    pub Descender: FT_Short,
    pub Line_Gap: FT_Short,
    pub advance_Width_Max: FT_UShort,
    pub min_Left_Side_Bearing: FT_Short,
    pub min_Right_Side_Bearing: FT_Short,
    pub xMax_Extent: FT_Short,
    pub caret_Slope_Rise: FT_Short,
    pub caret_Slope_Run: FT_Short,
    pub caret_Offset: FT_Short,
    pub Reserved: [FT_Short; 4],
    pub metric_Data_Format: FT_Short,
    pub number_Of_HMetrics: FT_UShort,
    pub long_metrics: FT_Pointer,
    pub short_metrics: FT_Pointer,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    pub achVendID: [FT_Char; 4],
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TT_Postscript {
    pub FormatType: FT_Fixed,
    pub italicAngle: FT_Fixed,
    pub underlinePosition: FT_Short,
    pub underlineThickness: FT_Short,
    pub isFixedPitch: FT_ULong,
    pub minMemType42: FT_ULong,
    pub maxMemType42: FT_ULong,
    pub minMemType1: FT_ULong,
    pub maxMemType1: FT_ULong,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TT_PCLT {
    pub Version: FT_Fixed,
    pub FontNumber: FT_ULong,
    pub Pitch: FT_UShort,
    pub xHeight: FT_UShort,
    pub Style: FT_UShort,
    pub TypeFamily: FT_UShort,
    pub CapHeight: FT_UShort,
    pub SymbolSet: FT_UShort,
    pub TypeFace: [FT_Char; 16],
    pub CharacterComplement: [FT_Char; 8],
    pub FileName: [FT_Char; 6],
    pub StrokeWeight: FT_Char,
    pub WidthType: FT_Char,
    pub SerifStyle: FT_Byte,
    pub Reserved: FT_Byte,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Vector {
    pub x: FT_Pos,
    pub y: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_BBox {
    pub xMin: FT_Pos,
    pub yMin: FT_Pos,
    pub xMax: FT_Pos,
    pub yMax: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Bitmap_C {
    pub rows: c_uint,
    pub width: c_uint,
    pub pitch: c_int,
    pub buffer: *mut FT_Byte,
    pub num_grays: c_ushort,
    pub pixel_mode: c_uchar,
    pub palette_mode: c_uchar,
    pub palette: FT_Pointer,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Outline {
    pub n_contours: FT_UShort,
    pub n_points: FT_UShort,
    pub points: *mut FT_Vector,
    pub tags: *mut FT_Byte,
    pub contours: *mut FT_UShort,
    pub flags: FT_Int,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FT_OutlineSnapshot {
    pub points: Vec<FT_Vector>,
    pub tags: Vec<FT_Byte>,
    pub contours: Vec<FT_UShort>,
    pub flags: FT_Int,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_GlyphRec {
    pub library: FT_Pointer,
    pub clazz: *const FT_Glyph_Class,
    pub format: FT_Glyph_Format,
    pub advance: FT_Vector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_GlyphCBoxSnapshot {
    pub has_class: bool,
    pub has_bbox_hook: bool,
    pub cbox: Option<FT_BBox>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FT_OutlineGlyphOwned {
    pub root: FT_GlyphRec,
    pub outline: FT_OutlineSnapshot,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_BitmapGlyphRec {
    pub root: FT_GlyphRec,
    pub left: FT_Int,
    pub top: FT_Int,
    pub bitmap: FT_Bitmap_C,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_OutlineGlyphRec {
    pub root: FT_GlyphRec,
    pub outline: FT_Outline,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_SvgGlyphRec {
    pub root: FT_GlyphRec,
    pub svg_document: *mut FT_Byte,
    pub svg_document_length: FT_ULong,
    pub glyph_index: FT_UInt,
    pub metrics: FT_Size_Metrics,
    pub units_per_EM: FT_UShort,
    pub start_glyph_id: FT_UShort,
    pub end_glyph_id: FT_UShort,
    pub transform: FT_Matrix,
    pub delta: FT_Vector,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct FT_GlyphSlotRecPublic {
    pub library: FT_Pointer,
    pub face: FT_Pointer,
    pub next: FT_Pointer,
    pub glyph_index: FT_UInt,
    pub generic: FT_Generic,
    pub metrics: FT_Glyph_Metrics,
    pub linearHoriAdvance: FT_Fixed,
    pub linearVertAdvance: FT_Fixed,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub bitmap: FT_Bitmap_C,
    pub bitmap_left: FT_Int,
    pub bitmap_top: FT_Int,
    pub outline: FT_Outline,
    pub num_subglyphs: FT_UInt,
    pub subglyphs: FT_SubGlyph,
    pub control_data: FT_Pointer,
    pub control_len: c_long,
    pub lsb_delta: FT_Pos,
    pub rsb_delta: FT_Pos,
    pub other: FT_Pointer,
    pub internal: FT_Slot_Internal,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Outline_Funcs {
    pub move_to: FT_Pointer,
    pub line_to: FT_Pointer,
    pub conic_to: FT_Pointer,
    pub cubic_to: FT_Pointer,
    pub shift: c_int,
    pub delta: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Span {
    pub x: c_ushort,
    pub len: c_ushort,
    pub coverage: c_uchar,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Raster_Params {
    pub target: *const FT_Bitmap_C,
    pub source: *const c_void,
    pub flags: c_int,
    pub gray_spans: FT_Pointer,
    pub black_spans: FT_Pointer,
    pub bit_test: FT_Pointer,
    pub bit_set: FT_Pointer,
    pub user: FT_Pointer,
    pub clip_box: FT_BBox,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Raster_Funcs {
    pub glyph_format: FT_Glyph_Format,
    pub raster_new: FT_Pointer,
    pub raster_reset: FT_Pointer,
    pub raster_set_mode: FT_Pointer,
    pub raster_render: FT_Pointer,
    pub raster_done: FT_Pointer,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Color {
    pub blue: FT_Byte,
    pub green: FT_Byte,
    pub red: FT_Byte,
    pub alpha: FT_Byte,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Palette_Data {
    pub num_palettes: FT_UShort,
    pub palette_name_ids: *const FT_UShort,
    pub palette_flags: *const FT_UShort,
    pub num_palette_entries: FT_UShort,
    pub palette_entry_name_ids: *const FT_UShort,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_LayerIterator {
    pub num_layers: FT_UInt,
    pub layer: FT_UInt,
    pub p: *mut FT_Byte,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_OpaquePaint {
    pub p: *mut FT_Byte,
    pub insert_root_transform: FT_Bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_ColorStopIterator {
    pub num_color_stops: FT_UInt,
    pub current_color_stop: FT_UInt,
    pub p: *mut FT_Byte,
    pub read_variable: FT_Bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_ColorIndex {
    pub palette_index: FT_UInt16,
    pub alpha: FT_F2Dot14,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_ColorStop {
    pub stop_offset: FT_Fixed,
    pub color: FT_ColorIndex,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_ColorLine {
    pub extend: FT_PaintExtend,
    pub color_stop_iterator: FT_ColorStopIterator,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Affine23 {
    pub xx: FT_Fixed,
    pub xy: FT_Fixed,
    pub dx: FT_Fixed,
    pub yx: FT_Fixed,
    pub yy: FT_Fixed,
    pub dy: FT_Fixed,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintColrLayers {
    pub layer_iterator: FT_LayerIterator,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintSolid {
    pub color: FT_ColorIndex,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintLinearGradient {
    pub colorline: FT_ColorLine,
    pub p0: FT_Vector,
    pub p1: FT_Vector,
    pub p2: FT_Vector,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintRadialGradient {
    pub colorline: FT_ColorLine,
    pub c0: FT_Vector,
    pub r0: FT_Pos,
    pub c1: FT_Vector,
    pub r1: FT_Pos,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintSweepGradient {
    pub colorline: FT_ColorLine,
    pub center: FT_Vector,
    pub start_angle: FT_Fixed,
    pub end_angle: FT_Fixed,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintGlyph {
    pub paint: FT_OpaquePaint,
    pub glyphID: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintColrGlyph {
    pub glyphID: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintTransform {
    pub paint: FT_OpaquePaint,
    pub affine: FT_Affine23,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintTranslate {
    pub paint: FT_OpaquePaint,
    pub dx: FT_Fixed,
    pub dy: FT_Fixed,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintScale {
    pub paint: FT_OpaquePaint,
    pub scale_x: FT_Fixed,
    pub scale_y: FT_Fixed,
    pub center_x: FT_Fixed,
    pub center_y: FT_Fixed,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintRotate {
    pub paint: FT_OpaquePaint,
    pub angle: FT_Fixed,
    pub center_x: FT_Fixed,
    pub center_y: FT_Fixed,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintSkew {
    pub paint: FT_OpaquePaint,
    pub x_skew_angle: FT_Fixed,
    pub y_skew_angle: FT_Fixed,
    pub center_x: FT_Fixed,
    pub center_y: FT_Fixed,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_PaintComposite {
    pub source_paint: FT_OpaquePaint,
    pub composite_mode: FT_Composite_Mode,
    pub backdrop_paint: FT_OpaquePaint,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_ClipBox {
    pub bottom_left: FT_Vector,
    pub top_left: FT_Vector,
    pub top_right: FT_Vector,
    pub bottom_right: FT_Vector,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_MM_Axis {
    pub name: *mut FT_String,
    pub minimum: FT_Long,
    pub maximum: FT_Long,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Multi_Master {
    pub num_axis: FT_UInt,
    pub num_designs: FT_UInt,
    pub axis: [FT_MM_Axis; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Var_Axis {
    pub name: *mut FT_String,
    pub minimum: FT_Fixed,
    pub def: FT_Fixed,
    pub maximum: FT_Fixed,
    pub tag: FT_ULong,
    pub strid: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Var_Named_Style {
    pub coords: *mut FT_Fixed,
    pub strid: FT_UInt,
    pub psid: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_MM_Var {
    pub num_axis: FT_UInt,
    pub num_designs: FT_UInt,
    pub num_namedstyles: FT_UInt,
    pub axis: *mut FT_Var_Axis,
    pub namedstyle: *mut FT_Var_Named_Style,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Prop_GlyphToScriptMap {
    pub face: FT_Pointer,
    pub map: *mut FT_UShort,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Prop_IncreaseXHeight {
    pub face: FT_Pointer,
    pub limit: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Incremental_MetricsRec {
    pub bearing_x: FT_Long,
    pub bearing_y: FT_Long,
    pub advance: FT_Long,
    pub advance_v: FT_Long,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Incremental_FuncsRec {
    pub get_glyph_data: FT_Pointer,
    pub free_glyph_data: FT_Pointer,
    pub get_glyph_metrics: FT_Pointer,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Incremental_InterfaceRec {
    pub funcs: *const FT_Incremental_FuncsRec,
    pub object: FT_Incremental,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_Renderer_Class {
    pub root: FT_Module_Class,
    pub glyph_format: FT_Glyph_Format,
    pub render_glyph: FT_Pointer,
    pub transform_glyph: FT_Pointer,
    pub get_glyph_cbox: FT_Pointer,
    pub set_mode: FT_Pointer,
    pub raster_class: *const FT_Raster_Funcs,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_SfntName {
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
    pub language_id: FT_UShort,
    pub name_id: FT_UShort,
    pub string: *mut FT_Byte,
    pub string_len: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FT_SfntLangTag {
    pub string: *mut FT_Byte,
    pub string_len: FT_UInt,
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FT_Bitmap {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub buffer: Vec<u8>,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
}
