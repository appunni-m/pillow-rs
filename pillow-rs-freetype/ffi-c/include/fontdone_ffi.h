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
typedef unsigned char FT_Byte;
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
typedef int FT_Size_Request_Type;
typedef int FT_Encoding;
typedef unsigned int FT_Sfnt_Tag;
typedef int FT_LcdFilter;
typedef int FT_TrueTypeEngineType;

typedef struct FT_LibraryRec_* FT_Library;
typedef struct FT_FaceRec_* FT_Face;
typedef struct FT_SizeRec_* FT_Size;
typedef struct FT_GlyphSlotRec_* FT_GlyphSlot;
typedef struct FT_CharMapRec_* FT_CharMap;

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

typedef struct FT_Bitmap_ {
  uint32_t rows;
  uint32_t width;
  FT_Int pitch;
  unsigned char* buffer;
  FT_UShort num_grays;
  FT_Pixel_Mode pixel_mode;
} FT_Bitmap;

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
FT_Error FT_Library_SetLcdFilter(FT_Library library, FT_LcdFilter filter);
FT_Error FT_Library_SetLcdFilterWeights(FT_Library library, FT_Byte* weights);
FT_Error FT_Library_SetLcdGeometry(FT_Library library, FT_Vector* sub);
FT_TrueTypeEngineType FT_Get_TrueType_Engine_Type(FT_Library library);
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
FT_Error FT_New_Memory_Face(FT_Library library, const unsigned char* file_base, FT_Long file_size, FT_Long face_index, FT_Face* aface);
FT_Error FT_Done_Face(FT_Face face);
FT_Bool FT_Face_CheckTrueTypePatents(FT_Face face);
FT_Bool FT_Face_SetUnpatentedHinting(FT_Face face, FT_Bool value);
FT_Error FT_Set_Char_Size(FT_Face face, FT_F26Dot6 char_width, FT_F26Dot6 char_height, FT_UInt horz_resolution, FT_UInt vert_resolution);
FT_Error FT_Set_Pixel_Sizes(FT_Face face, FT_UInt pixel_width, FT_UInt pixel_height);
FT_Error FT_Request_Size(FT_Face face, FT_Size_Request req);
FT_UInt FT_Get_Char_Index(FT_Face face, FT_ULong char_code);
FT_Error FT_Get_Kerning(FT_Face face, FT_UInt left_glyph, FT_UInt right_glyph, FT_UInt kern_mode, FT_Vector* akerning);
FT_Error FT_Select_Charmap(FT_Face face, FT_Encoding encoding);
FT_Error FT_Set_Charmap(FT_Face face, FT_CharMap charmap);
FT_Int FT_Get_Charmap_Index(FT_CharMap charmap);
FT_Long FT_Get_CMap_Format(FT_CharMap charmap);
FT_ULong FT_Get_CMap_Language_ID(FT_CharMap charmap);
FT_UShort FT_Get_FSType_Flags(FT_Face face);
FT_Int FT_Get_Gasp(FT_Face face, FT_UInt ppem);
FT_Error FT_Get_Glyph_Name(FT_Face face, FT_UInt glyph_index, void* buffer, FT_UInt buffer_max);
FT_UInt FT_Get_Name_Index(FT_Face face, const char* glyph_name);
const char* FT_Get_Postscript_Name(FT_Face face);
FT_UInt FT_Get_Sfnt_Name_Count(FT_Face face);
FT_Error FT_Get_Sfnt_Name(FT_Face face, FT_UInt idx, FT_SfntName* aname);
void* FT_Get_Sfnt_Table(FT_Face face, FT_Sfnt_Tag tag);
FT_Error FT_Load_Sfnt_Table(FT_Face face, FT_ULong tag, FT_Long offset, FT_Byte* buffer, FT_ULong* length);
FT_Error FT_Sfnt_Table_Info(FT_Face face, FT_UInt table_index, FT_ULong* tag, FT_ULong* length);
FT_ULong FT_Get_First_Char(FT_Face face, FT_UInt* agindex);
FT_ULong FT_Get_Next_Char(FT_Face face, FT_ULong char_code, FT_UInt* agindex);
void FT_Library_Version(FT_Library library, FT_Int* amajor, FT_Int* aminor, FT_Int* apatch);
FT_Error FT_Load_Char(FT_Face face, FT_ULong char_code, FT_Int32 load_flags);
FT_Error FT_Load_Glyph(FT_Face face, FT_UInt glyph_index, FT_Int32 load_flags);
FT_Error FT_Get_Advance(FT_Face face, FT_UInt glyph_index, FT_Int32 load_flags, FT_Fixed* padvance);
FT_Error FT_Get_Advances(FT_Face face, FT_UInt start, FT_UInt count, FT_Int32 load_flags, FT_Fixed* padvances);
FT_Error FT_Get_SubGlyph_Info(FT_GlyphSlot glyph, FT_UInt sub_index, FT_Int* p_index, FT_UInt* p_flags, FT_Int* p_arg1, FT_Int* p_arg2, FT_Matrix* p_transform);
FT_Error FT_Render_Glyph(FT_GlyphSlot slot, FT_Render_Mode render_mode);

#ifdef __cplusplus
}
#endif

#endif
