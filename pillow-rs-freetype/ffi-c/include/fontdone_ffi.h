#ifndef FONTDONE_FFI_H
#define FONTDONE_FFI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int32_t FT_Error;
typedef int FT_Int;
typedef unsigned int FT_UInt;
typedef int32_t FT_Int32;
typedef long FT_Long;
typedef unsigned long FT_ULong;
typedef long FT_Pos;
typedef long FT_Fixed;
typedef long FT_F26Dot6;
typedef unsigned short FT_UShort;
typedef int FT_Render_Mode;
typedef int FT_Pixel_Mode;
typedef int FT_Glyph_Format;

typedef struct FT_LibraryRec_* FT_Library;
typedef struct FT_FaceRec_* FT_Face;
typedef struct FT_SizeRec_* FT_Size;
typedef struct FT_GlyphSlotRec_* FT_GlyphSlot;

typedef struct FT_Vector_ {
  FT_Pos x;
  FT_Pos y;
} FT_Vector;

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

typedef struct FT_Bitmap_ {
  uint32_t rows;
  uint32_t width;
  FT_Int pitch;
  unsigned char* buffer;
  FT_UShort num_grays;
  FT_Pixel_Mode pixel_mode;
} FT_Bitmap;

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
FT_Error FT_New_Memory_Face(FT_Library library, const unsigned char* file_base, FT_Long file_size, FT_Long face_index, FT_Face* aface);
FT_Error FT_Done_Face(FT_Face face);
FT_Error FT_Set_Char_Size(FT_Face face, FT_F26Dot6 char_width, FT_F26Dot6 char_height, FT_UInt horz_resolution, FT_UInt vert_resolution);
FT_Error FT_Set_Pixel_Sizes(FT_Face face, FT_UInt pixel_width, FT_UInt pixel_height);
FT_UInt FT_Get_Char_Index(FT_Face face, FT_ULong char_code);
FT_Error FT_Load_Char(FT_Face face, FT_ULong char_code, FT_Int32 load_flags);
FT_Error FT_Load_Glyph(FT_Face face, FT_UInt glyph_index, FT_Int32 load_flags);
FT_Error FT_Get_Advance(FT_Face face, FT_UInt glyph_index, FT_Int32 load_flags, FT_Fixed* padvance);
FT_Error FT_Get_Advances(FT_Face face, FT_UInt start, FT_UInt count, FT_Int32 load_flags, FT_Fixed* padvances);
FT_Error FT_Render_Glyph(FT_GlyphSlot slot, FT_Render_Mode render_mode);

#ifdef __cplusplus
}
#endif

#endif
