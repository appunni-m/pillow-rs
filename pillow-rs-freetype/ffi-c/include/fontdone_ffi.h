#ifndef FONTDONE_FFI_H
#define FONTDONE_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int32_t FT_Error;
typedef unsigned char FT_Bool;
typedef int FT_Int;
typedef unsigned int FT_UInt;
typedef int32_t FT_Int32;
typedef uint32_t FT_UInt32;
typedef unsigned char FT_Byte;
typedef const FT_Byte* FT_Bytes;
typedef long FT_Long;
typedef unsigned long FT_ULong;
typedef long FT_Pos;
typedef long FT_Fixed;
typedef long FT_Angle;
typedef long FT_F26Dot6;
typedef short FT_Short;
typedef unsigned short FT_UShort;
typedef int FT_Render_Mode;
typedef int FT_Pixel_Mode;
typedef int FT_Glyph_Format;
typedef int FT_Orientation;
typedef int FT_Size_Request_Type;
typedef int FT_Encoding;
typedef unsigned int FT_Sfnt_Tag;
typedef int FT_LcdFilter;
typedef int FT_TrueTypeEngineType;
typedef char FT_String;
typedef int FT_StrokerBorder;
typedef FT_Error (*FT_DebugHook_Func)(void* arg);
typedef void* FT_Pointer;
typedef FT_Pointer FT_Module_Interface;
typedef FT_Pointer FT_Generic_Finalizer;

typedef struct FT_LibraryRec_* FT_Library;
typedef struct FT_FaceRec_* FT_Face;
typedef struct FT_SizeRec_* FT_Size;
typedef struct FT_GlyphSlotRec_* FT_GlyphSlot;
typedef struct FT_GlyphRec_* FT_Glyph;
typedef struct FT_CharMapRec_* FT_CharMap;
typedef struct FT_StrokerRec_* FT_Stroker;
typedef struct FT_ListNodeRec_* FT_ListNode;
typedef struct FT_ListRec_* FT_List;
typedef struct FT_MemoryRec_* FT_Memory;

typedef struct FT_ListNodeRec_ {
  FT_ListNode prev;
  FT_ListNode next;
  FT_Pointer data;
} FT_ListNodeRec;

typedef struct FT_ListRec_ {
  FT_ListNode head;
  FT_ListNode tail;
} FT_ListRec;

typedef struct FT_Generic_ {
  FT_Pointer data;
  FT_Generic_Finalizer finalizer;
} FT_Generic;

typedef FT_Error (*FT_List_Iterator)(FT_ListNode node, void* user);
typedef void* (*FT_Alloc_Func)(FT_Memory memory, long size);
typedef void (*FT_Free_Func)(FT_Memory memory, void* block);
typedef void* (*FT_Realloc_Func)(FT_Memory memory, long cur_size, long new_size, void* block);
typedef void (*FT_List_Destructor)(FT_Memory memory, void* data, void* user);

typedef struct FT_MemoryRec_ {
  void* user;
  FT_Alloc_Func alloc;
  FT_Free_Func free;
  FT_Realloc_Func realloc;
} FT_MemoryRec;

typedef struct FT_Vector_ {
  FT_Pos x;
  FT_Pos y;
} FT_Vector;

typedef struct FT_Matrix_ {
  FT_Fixed xx;
  FT_Fixed xy;
  FT_Fixed yx;
  FT_Fixed yy;
} FT_Matrix;

typedef struct FT_BBox_ {
  FT_Pos xMin;
  FT_Pos yMin;
  FT_Pos xMax;
  FT_Pos yMax;
} FT_BBox;

typedef struct FT_Parameter_ {
  FT_ULong tag;
  void* data;
} FT_Parameter;

typedef struct FT_MM_Axis_ {
  char* name;
  FT_Long minimum;
  FT_Long maximum;
} FT_MM_Axis;

typedef struct FT_Multi_Master_ {
  FT_UInt num_axis;
  FT_UInt num_designs;
  FT_MM_Axis axis[4];
} FT_Multi_Master;

typedef struct FT_Var_Axis_ {
  char* name;
  FT_Fixed minimum;
  FT_Fixed def;
  FT_Fixed maximum;
  FT_ULong tag;
  FT_UInt strid;
} FT_Var_Axis;

typedef struct FT_Var_Named_Style_ {
  FT_Fixed* coords;
  FT_UInt strid;
  FT_UInt psid;
} FT_Var_Named_Style;

typedef struct FT_MM_Var_ {
  FT_UInt num_axis;
  FT_UInt num_designs;
  FT_UInt num_namedstyles;
  FT_Var_Axis* axis;
  FT_Var_Named_Style* namedstyle;
} FT_MM_Var;

typedef struct FT_WinFNT_HeaderRec_ {
  FT_UShort version;
  FT_ULong file_size;
  FT_Byte copyright[60];
  FT_UShort file_type;
  FT_UShort nominal_point_size;
  FT_UShort vertical_resolution;
  FT_UShort horizontal_resolution;
  FT_UShort ascent;
  FT_UShort internal_leading;
  FT_UShort external_leading;
  FT_Byte italic;
  FT_Byte underline;
  FT_Byte strike_out;
  FT_UShort weight;
  FT_Byte charset;
  FT_UShort pixel_width;
  FT_UShort pixel_height;
  FT_Byte pitch_and_family;
  FT_UShort avg_width;
  FT_UShort max_width;
  FT_Byte first_char;
  FT_Byte last_char;
  FT_Byte default_char;
  FT_Byte break_char;
  FT_UShort bytes_per_row;
  FT_ULong device_offset;
  FT_ULong face_name_offset;
  FT_ULong bits_pointer;
  FT_ULong bits_offset;
  FT_Byte reserved;
  FT_ULong flags;
  FT_UShort A_space;
  FT_UShort B_space;
  FT_UShort C_space;
  FT_UShort color_table_offset;
  FT_ULong reserved1[4];
} FT_WinFNT_HeaderRec;

typedef FT_WinFNT_HeaderRec* FT_WinFNT_Header;

typedef enum BDF_PropertyType_ {
  BDF_PROPERTY_TYPE_NONE = 0,
  BDF_PROPERTY_TYPE_ATOM = 1,
  BDF_PROPERTY_TYPE_INTEGER = 2,
  BDF_PROPERTY_TYPE_CARDINAL = 3
} BDF_PropertyType;

typedef struct BDF_PropertyRec_ {
  BDF_PropertyType type;
  union {
    const char* atom;
    FT_Int32 integer;
    FT_UInt32 cardinal;
  } u;
} BDF_PropertyRec;

typedef BDF_PropertyRec* BDF_Property;

typedef enum T1_EncodingType_ {
  T1_ENCODING_TYPE_NONE = 0,
  T1_ENCODING_TYPE_ARRAY = 1,
  T1_ENCODING_TYPE_STANDARD = 2,
  T1_ENCODING_TYPE_ISOLATIN1 = 3,
  T1_ENCODING_TYPE_EXPERT = 4
} T1_EncodingType;

typedef enum PS_Dict_Keys_ {
  PS_DICT_ENCODING_TYPE = 9,
  PS_DICT_ENCODING_ENTRY = 10
} PS_Dict_Keys;

typedef struct PS_FontInfoRec_ {
  char* version;
  char* notice;
  char* full_name;
  char* family_name;
  char* weight;
  FT_Fixed italic_angle;
  FT_Bool is_fixed_pitch;
  FT_Short underline_position;
  FT_UShort underline_thickness;
} PS_FontInfoRec;

typedef PS_FontInfoRec* PS_FontInfo;
typedef PS_FontInfoRec T1_FontInfo;

typedef struct PS_PrivateRec_ {
  FT_Int unique_id;
  FT_Int lenIV;
  FT_Byte num_blue_values;
  FT_Byte num_other_blues;
  FT_Byte num_family_blues;
  FT_Byte num_family_other_blues;
  FT_Short blue_values[14];
  FT_Short other_blues[10];
  FT_Short family_blues[14];
  FT_Short family_other_blues[10];
  FT_Fixed blue_scale;
  FT_Int blue_shift;
  FT_Int blue_fuzz;
  FT_UShort standard_width[1];
  FT_UShort standard_height[1];
  FT_Byte num_snap_widths;
  FT_Byte num_snap_heights;
  FT_Bool force_bold;
  FT_Bool round_stem_up;
  FT_Short snap_widths[13];
  FT_Short snap_heights[13];
  FT_Fixed expansion_factor;
  FT_Long language_group;
  FT_Long password;
  FT_Short min_feature[2];
} PS_PrivateRec;

typedef PS_PrivateRec* PS_Private;
typedef PS_PrivateRec T1_Private;

typedef struct FT_Open_Args_ {
  FT_UInt flags;
  const FT_Byte* memory_base;
  FT_Long memory_size;
  char* pathname;
  void* stream;
  void* driver;
  FT_Int num_params;
  FT_Parameter* params;
} FT_Open_Args;

typedef struct FT_Outline_ {
  FT_UShort n_contours;
  FT_UShort n_points;
  FT_Vector* points;
  FT_Byte* tags;
  FT_UShort* contours;
  FT_Int flags;
} FT_Outline;

typedef struct FT_Glyph_Class_ {
  FT_Long glyph_size;
  FT_Glyph_Format glyph_format;
  FT_Pointer glyph_init;
  FT_Pointer glyph_done;
  FT_Pointer glyph_copy;
  FT_Pointer glyph_transform;
  FT_Pointer glyph_bbox;
  FT_Pointer glyph_prepare;
} FT_Glyph_Class;

typedef struct FT_GlyphRec_ {
  FT_Pointer library;
  const FT_Glyph_Class* clazz;
  FT_Glyph_Format format;
  FT_Vector advance;
} FT_GlyphRec;

typedef struct FT_Glyph_Metrics_ {
  FT_Pos width;
  FT_Pos height;
  FT_Pos horiBearingX;
  FT_Pos horiBearingY;
  FT_Pos horiAdvance;
  FT_Pos vertBearingX;
  FT_Pos vertBearingY;
  FT_Pos vertAdvance;
} FT_Glyph_Metrics;

typedef struct FT_Size_Metrics_ {
  FT_UShort x_ppem;
  FT_UShort y_ppem;
  FT_Fixed x_scale;
  FT_Fixed y_scale;
  FT_Pos ascender;
  FT_Pos descender;
  FT_Pos height;
  FT_Pos max_advance;
} FT_Size_Metrics;

typedef struct FT_Size_RequestRec_ {
  FT_Size_Request_Type type;
  FT_Long width;
  FT_Long height;
  FT_UInt horiResolution;
  FT_UInt vertResolution;
} FT_Size_RequestRec;
typedef FT_Size_RequestRec* FT_Size_Request;

const char* FT_Error_String(FT_Error error_code);

typedef struct FT_Bitmap_ {
  uint32_t rows;
  uint32_t width;
  FT_Int pitch;
  unsigned char* buffer;
  FT_UShort num_grays;
  FT_Pixel_Mode pixel_mode;
  unsigned char palette_mode;
  void* palette;
} FT_Bitmap;

typedef struct FT_Span_ {
  short x;
  unsigned short len;
  unsigned char coverage;
} FT_Span;

typedef void (*FT_SpanFunc)(int y, int count, const FT_Span* spans, void* user);
typedef int (*FT_Raster_BitTest_Func)(int y, int x, void* user);
typedef void (*FT_Raster_BitSet_Func)(int y, int x, void* user);

typedef struct FT_Raster_Params_ {
  const FT_Bitmap* target;
  const void* source;
  int flags;
  FT_SpanFunc gray_spans;
  FT_SpanFunc black_spans;
  FT_Raster_BitTest_Func bit_test;
  FT_Raster_BitSet_Func bit_set;
  void* user;
  FT_BBox clip_box;
} FT_Raster_Params;

typedef struct FT_Color_ {
  FT_Byte blue;
  FT_Byte green;
  FT_Byte red;
  FT_Byte alpha;
} FT_Color;

typedef struct FT_Palette_Data_ {
  FT_UShort num_palettes;
  const FT_UShort* palette_name_ids;
  const FT_UShort* palette_flags;
  FT_UShort num_palette_entries;
  const FT_UShort* palette_entry_name_ids;
} FT_Palette_Data;

void FT_Bitmap_Init(FT_Bitmap* abitmap);
void FT_Bitmap_New(FT_Bitmap* abitmap);
FT_Error FT_Bitmap_Copy(FT_Library library, const FT_Bitmap* source, FT_Bitmap* target);
FT_Error FT_Bitmap_Convert(FT_Library library, const FT_Bitmap* source, FT_Bitmap* target, FT_Int alignment);
FT_Error FT_Bitmap_Done(FT_Library library, FT_Bitmap* bitmap);
FT_Error FT_Bitmap_Embolden(FT_Library library, FT_Bitmap* bitmap, FT_Pos xStrength, FT_Pos yStrength);
FT_Error FT_Bitmap_Blend(FT_Library library, const FT_Bitmap* source, FT_Vector source_offset, FT_Bitmap* target, FT_Vector* atarget_offset, FT_Color color);
FT_Error FT_GlyphSlot_Own_Bitmap(FT_GlyphSlot slot);
FT_Error FT_Palette_Data_Get(FT_Face face, FT_Palette_Data* apalette_data);
FT_Error FT_Palette_Select(FT_Face face, FT_UShort palette_index, FT_Color** apalette);
FT_Error FT_Palette_Set_Foreground_Color(FT_Face face, FT_Color foreground_color);
void FT_TrueTypeGX_Free(FT_Face face, FT_Bytes table);
void FT_ClassicKern_Free(FT_Face face, FT_Bytes table);

typedef struct FT_SfntName_ {
  FT_UShort platform_id;
  FT_UShort encoding_id;
  FT_UShort language_id;
  FT_UShort name_id;
  FT_Byte* string;
  FT_UInt string_len;
} FT_SfntName;

struct FT_CharMapRec_ {
  FT_Face face;
  FT_Encoding encoding;
  FT_UShort platform_id;
  FT_UShort encoding_id;
};

typedef struct TT_OS2_ {
  FT_UShort version;
  FT_Short xAvgCharWidth;
  FT_UShort usWeightClass;
  FT_UShort usWidthClass;
  FT_UShort fsType;
  FT_Short ySubscriptXSize;
  FT_Short ySubscriptYSize;
  FT_Short ySubscriptXOffset;
  FT_Short ySubscriptYOffset;
  FT_Short ySuperscriptXSize;
  FT_Short ySuperscriptYSize;
  FT_Short ySuperscriptXOffset;
  FT_Short ySuperscriptYOffset;
  FT_Short yStrikeoutSize;
  FT_Short yStrikeoutPosition;
  FT_Short sFamilyClass;
  FT_Byte panose[10];
  FT_ULong ulUnicodeRange1;
  FT_ULong ulUnicodeRange2;
  FT_ULong ulUnicodeRange3;
  FT_ULong ulUnicodeRange4;
  signed char achVendID[4];
  FT_UShort fsSelection;
  FT_UShort usFirstCharIndex;
  FT_UShort usLastCharIndex;
  FT_Short sTypoAscender;
  FT_Short sTypoDescender;
  FT_Short sTypoLineGap;
  FT_UShort usWinAscent;
  FT_UShort usWinDescent;
  FT_ULong ulCodePageRange1;
  FT_ULong ulCodePageRange2;
  FT_Short sxHeight;
  FT_Short sCapHeight;
  FT_UShort usDefaultChar;
  FT_UShort usBreakChar;
  FT_UShort usMaxContext;
  FT_UShort usLowerOpticalPointSize;
  FT_UShort usUpperOpticalPointSize;
} TT_OS2;

typedef struct TT_VertHeader_ {
  FT_Fixed Version;
  FT_Short Ascender;
  FT_Short Descender;
  FT_Short Line_Gap;
  FT_UShort advance_Height_Max;
  FT_Short min_Top_Side_Bearing;
  FT_Short min_Bottom_Side_Bearing;
  FT_Short yMax_Extent;
  FT_Short caret_Slope_Rise;
  FT_Short caret_Slope_Run;
  FT_Short caret_Offset;
  FT_Short Reserved[4];
  FT_Short metric_Data_Format;
  FT_UShort number_Of_VMetrics;
  void* long_metrics;
  void* short_metrics;
} TT_VertHeader;

struct FT_GlyphSlotRec_ {
  FT_UInt glyph_index;
  FT_Glyph_Metrics metrics;
  FT_Vector advance;
  FT_Glyph_Format format;
  FT_Bitmap bitmap;
  FT_Int bitmap_left;
  FT_Int bitmap_top;
};

struct FT_SizeRec_ {
  FT_Face face;
  FT_Generic generic;
  FT_Size_Metrics metrics;
  void* internal;
};

struct FT_FaceRec_ {
  FT_GlyphSlot glyph;
  FT_Size size;
  void* internal;
};

struct FT_LibraryRec_ {
  void* internal;
};

FT_Error FT_Init_FreeType(FT_Library* alibrary);
FT_Error FT_Done_FreeType(FT_Library library);
FT_Error FT_New_Library(FT_Memory memory, FT_Library* alibrary);
FT_Error FT_Reference_Library(FT_Library library);
FT_Error FT_Done_Library(FT_Library library);
FT_Error FT_Get_MM_Var(FT_Face face, FT_MM_Var** amaster);
FT_Error FT_Done_MM_Var(FT_Library library, FT_MM_Var* amaster);
FT_Error FT_Get_Var_Axis_Flags(FT_MM_Var* master, FT_UInt axis_index, FT_UInt* flags);
FT_Error FT_Library_SetLcdFilter(FT_Library library, FT_LcdFilter filter);
FT_Error FT_Library_SetLcdFilterWeights(FT_Library library, FT_Byte* weights);
FT_Error FT_Library_SetLcdGeometry(FT_Library library, FT_Vector* sub);
FT_TrueTypeEngineType FT_Get_TrueType_Engine_Type(FT_Library library);
FT_Error FT_Property_Get(FT_Library library, const FT_String* module_name, const FT_String* property_name, void* value);
FT_Error FT_Property_Set(FT_Library library, const FT_String* module_name, const FT_String* property_name, const void* value);
void FT_Set_Default_Properties(FT_Library library);
FT_Error FT_Face_Properties(FT_Face face, FT_UInt num_properties, FT_Parameter* properties);
void FT_Add_Default_Modules(FT_Library library);
void FT_Set_Debug_Hook(FT_Library library, FT_UInt hook_index, FT_DebugHook_Func debug_hook);
FT_Long FT_MulDiv(FT_Long a, FT_Long b, FT_Long c);
FT_Long FT_MulFix(FT_Long a, FT_Long b);
FT_Long FT_DivFix(FT_Long a, FT_Long b);
FT_Fixed FT_RoundFix(FT_Fixed a);
FT_Fixed FT_CeilFix(FT_Fixed a);
FT_Fixed FT_FloorFix(FT_Fixed a);
FT_Fixed FT_Sin(FT_Angle angle);
FT_Fixed FT_Cos(FT_Angle angle);
FT_Fixed FT_Tan(FT_Angle angle);
FT_Angle FT_Atan2(FT_Fixed dx, FT_Fixed dy);
FT_Angle FT_Angle_Diff(FT_Angle angle1, FT_Angle angle2);
void FT_Vector_Unit(FT_Vector* vector, FT_Angle angle);
void FT_Vector_Rotate(FT_Vector* vector, FT_Angle angle);
FT_Fixed FT_Vector_Length(FT_Vector* vector);
void FT_Vector_Polarize(FT_Vector* vector, FT_Fixed* length, FT_Angle* angle);
void FT_Vector_From_Polar(FT_Vector* vector, FT_Fixed length, FT_Angle angle);
void FT_Vector_Transform(FT_Vector* vector, const FT_Matrix* matrix);
void FT_Matrix_Multiply(const FT_Matrix* a, FT_Matrix* b);
FT_Error FT_Matrix_Invert(FT_Matrix* matrix);
FT_Error FT_Open_Face(FT_Library library, const FT_Open_Args* args, FT_Long face_index, FT_Face* aface);
FT_Error FT_New_Memory_Face(FT_Library library, const unsigned char* file_base, FT_Long file_size, FT_Long face_index, FT_Face* aface);
FT_Error FT_Done_Face(FT_Face face);
FT_Error FT_New_Size(FT_Face face, FT_Size* asize);
FT_Error FT_Done_Size(FT_Size size);
FT_Error FT_Activate_Size(FT_Size size);
FT_Bool FT_Face_CheckTrueTypePatents(FT_Face face);
FT_Bool FT_Face_SetUnpatentedHinting(FT_Face face, FT_Bool value);
void FT_Outline_Get_CBox(const FT_Outline* outline, FT_BBox* acbox);
void FT_Glyph_Get_CBox(FT_Glyph glyph, FT_UInt bbox_mode, FT_BBox* acbox);
FT_Error FT_Get_Glyph(FT_GlyphSlot slot, FT_Glyph* aglyph);
FT_Error FT_Glyph_Copy(FT_Glyph source, FT_Glyph* target);
void FT_Done_Glyph(FT_Glyph glyph);
FT_Error FT_Glyph_Transform(FT_Glyph glyph, const FT_Matrix* matrix, const FT_Vector* delta);
FT_Error FT_Glyph_To_Bitmap(FT_Glyph* the_glyph, FT_Render_Mode render_mode, const FT_Vector* origin, FT_Bool destroy);
FT_Error FT_Outline_Get_BBox(const FT_Outline* outline, FT_BBox* abbox);
FT_Error FT_Outline_Get_Bitmap(FT_Library library, const FT_Outline* outline, FT_Bitmap* abitmap);
FT_Error FT_Outline_Render(FT_Library library, FT_Outline* outline, FT_Raster_Params* params);
FT_Error FT_Outline_Check(const FT_Outline* outline);
FT_Error FT_Outline_Copy(const FT_Outline* source, FT_Outline* target);
FT_Error FT_Outline_New(FT_Library library, FT_UInt numPoints, FT_Int numContours, FT_Outline* anoutline);
FT_Error FT_Outline_Done(FT_Library library, FT_Outline* outline);
FT_Error FT_Outline_Embolden(FT_Outline* outline, FT_Pos strength);
FT_Error FT_Outline_EmboldenXY(FT_Outline* outline, FT_Pos xstrength, FT_Pos ystrength);
FT_StrokerBorder FT_Outline_GetInsideBorder(const FT_Outline* outline);
FT_StrokerBorder FT_Outline_GetOutsideBorder(const FT_Outline* outline);
void FT_Stroker_Set(FT_Stroker stroker, FT_Fixed radius, int line_cap, int line_join, FT_Fixed miter_limit);
void FT_Stroker_Rewind(FT_Stroker stroker);
void FT_Stroker_Done(FT_Stroker stroker);
FT_Orientation FT_Outline_Get_Orientation(const FT_Outline* outline);
void FT_Outline_Reverse(FT_Outline* outline);
void FT_Outline_Transform(const FT_Outline* outline, const FT_Matrix* matrix);
void FT_Outline_Translate(const FT_Outline* outline, FT_Pos xOffset, FT_Pos yOffset);
FT_Error FT_Set_Char_Size(FT_Face face, FT_F26Dot6 char_width, FT_F26Dot6 char_height, FT_UInt horz_resolution, FT_UInt vert_resolution);
FT_Error FT_Set_Pixel_Sizes(FT_Face face, FT_UInt pixel_width, FT_UInt pixel_height);
FT_Error FT_Request_Size(FT_Face face, FT_Size_Request req);
FT_Error FT_Select_Size(FT_Face face, FT_Int strike_index);
FT_UInt FT_Get_Char_Index(FT_Face face, FT_ULong char_code);
FT_UInt FT_Face_GetCharVariantIndex(FT_Face face, FT_ULong charcode, FT_ULong variant_selector);
FT_Int FT_Face_GetCharVariantIsDefault(FT_Face face, FT_ULong charcode, FT_ULong variant_selector);
FT_UInt32* FT_Face_GetVariantSelectors(FT_Face face);
FT_UInt32* FT_Face_GetVariantsOfChar(FT_Face face, FT_ULong charcode);
FT_UInt32* FT_Face_GetCharsOfVariant(FT_Face face, FT_ULong variant_selector);
FT_Error FT_Get_Kerning(FT_Face face, FT_UInt left_glyph, FT_UInt right_glyph, FT_UInt kern_mode, FT_Vector* akerning);
FT_Error FT_Get_PFR_Kerning(FT_Face face, FT_UInt left_glyph, FT_UInt right_glyph, FT_Vector* avector);
FT_Error FT_Select_Charmap(FT_Face face, FT_Encoding encoding);
FT_Error FT_Set_Charmap(FT_Face face, FT_CharMap charmap);
FT_Int FT_Get_Charmap_Index(FT_CharMap charmap);
FT_Long FT_Get_CMap_Format(FT_CharMap charmap);
FT_ULong FT_Get_CMap_Language_ID(FT_CharMap charmap);
FT_UShort FT_Get_FSType_Flags(FT_Face face);
FT_Int FT_Get_Gasp(FT_Face face, FT_UInt ppem);
void FT_List_Add(FT_List list, FT_ListNode node);
void FT_List_Insert(FT_List list, FT_ListNode node);
FT_ListNode FT_List_Find(FT_List list, void* data);
void FT_List_Remove(FT_List list, FT_ListNode node);
void FT_List_Up(FT_List list, FT_ListNode node);
FT_Error FT_List_Iterate(FT_List list, FT_List_Iterator iterator, void* user);
void FT_List_Finalize(FT_List list, FT_List_Destructor destroy, FT_Memory memory, void* user);
FT_Error FT_Get_Glyph_Name(FT_Face face, FT_UInt glyph_index, void* buffer, FT_UInt buffer_max);
FT_UInt FT_Get_Name_Index(FT_Face face, const char* glyph_name);
const char* FT_Get_Postscript_Name(FT_Face face);
const char* FT_Get_Font_Format(FT_Face face);
const char* FT_Get_X11_Font_Format(FT_Face face);
FT_Error FT_Set_Named_Instance(FT_Face face, FT_UInt instance_index);
FT_Error FT_Get_MM_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Get_Multi_Master(FT_Face face, FT_Multi_Master* amaster);
FT_Error FT_Set_MM_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Long* coords);
FT_Error FT_Set_MM_WeightVector(FT_Face face, FT_UInt len, FT_Fixed* weightvector);
FT_Error FT_Get_MM_WeightVector(FT_Face face, FT_UInt* len, FT_Fixed* weightvector);
FT_Error FT_Get_Var_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Get_Var_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Set_MM_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Set_Var_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Set_Var_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Get_Default_Named_Instance(FT_Face face, FT_UInt* instance_index);
FT_Error FT_Get_WinFNT_Header(FT_Face face, FT_WinFNT_HeaderRec* aheader);
FT_Error FT_Get_BDF_Property(FT_Face face, const char* prop_name, BDF_PropertyRec* aproperty);
FT_Error FT_Get_BDF_Charset_ID(FT_Face face, const char** acharset_encoding, const char** acharset_registry);
FT_Error FT_Get_PS_Font_Info(FT_Face face, PS_FontInfo afont_info);
FT_Error FT_Get_PS_Font_Private(FT_Face face, PS_Private afont_private);
FT_Long FT_Get_PS_Font_Value(FT_Face face, PS_Dict_Keys key, FT_UInt idx, void* value, FT_Long value_len);
FT_UInt FT_Get_Sfnt_Name_Count(FT_Face face);
FT_Error FT_Get_Sfnt_Name(FT_Face face, FT_UInt idx, FT_SfntName* aname);
void* FT_Get_Sfnt_Table(FT_Face face, FT_Sfnt_Tag tag);
FT_Error FT_Load_Sfnt_Table(FT_Face face, FT_ULong tag, FT_Long offset, FT_Byte* buffer, FT_ULong* length);
FT_Error FT_Sfnt_Table_Info(FT_Face face, FT_UInt table_index, FT_ULong* tag, FT_ULong* length);
FT_Error FT_OpenType_Validate(FT_Face face, FT_UInt validation_flags, FT_Bytes* BASE_table, FT_Bytes* GDEF_table, FT_Bytes* GPOS_table, FT_Bytes* GSUB_table, FT_Bytes* JSTF_table);
void FT_OpenType_Free(FT_Face face, FT_Bytes table);
FT_ULong FT_Get_First_Char(FT_Face face, FT_UInt* agindex);
FT_ULong FT_Get_Next_Char(FT_Face face, FT_ULong char_code, FT_UInt* agindex);
void FT_Library_Version(FT_Library library, FT_Int* amajor, FT_Int* aminor, FT_Int* apatch);
FT_Error FT_Load_Char(FT_Face face, FT_ULong char_code, FT_Int32 load_flags);
FT_Error FT_Load_Glyph(FT_Face face, FT_UInt glyph_index, FT_Int32 load_flags);
FT_Error FT_Get_Advance(FT_Face face, FT_UInt glyph_index, FT_Int32 load_flags, FT_Fixed* padvance);
FT_Error FT_Get_Advances(FT_Face face, FT_UInt start, FT_UInt count, FT_Int32 load_flags, FT_Fixed* padvances);
FT_Error FT_Get_SubGlyph_Info(FT_GlyphSlot glyph, FT_UInt sub_index, FT_Int* p_index, FT_UInt* p_flags, FT_Int* p_arg1, FT_Int* p_arg2, FT_Matrix* p_transform);
FT_Error FT_Render_Glyph(FT_GlyphSlot slot, FT_Render_Mode render_mode);
void FT_GlyphSlot_AdjustWeight(FT_GlyphSlot slot, FT_Fixed xdelta, FT_Fixed ydelta);
void FT_GlyphSlot_Embolden(FT_GlyphSlot slot);
void FT_GlyphSlot_Oblique(FT_GlyphSlot slot);
void FT_GlyphSlot_Slant(FT_GlyphSlot slot, FT_Fixed xslant, FT_Fixed yslant);

#ifdef __cplusplus
}
#endif

#endif
