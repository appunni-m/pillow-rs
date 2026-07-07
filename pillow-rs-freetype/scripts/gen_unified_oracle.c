#include <ft2build.h>
#include FT_FREETYPE_H

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <freetype/ftcache.h>
#include <freetype/ftadvanc.h>
#include <freetype/ftcolor.h>
#include <freetype/ftdriver.h>
#include <freetype/ftglyph.h>
#include <freetype/ftimage.h>
#include <freetype/ftincrem.h>
#include <freetype/ftmm.h>
#include <freetype/ftmodapi.h>
#include <freetype/ftrender.h>
#include <freetype/ftsnames.h>
#include <freetype/ftsystem.h>
#include <freetype/t1tables.h>
#include <freetype/tttables.h>

#ifndef FT_ERR_PREFIX
#define FT_ERR_PREFIX FT_Err_
#endif

static int streq(const char* a, const char* b) {
    return strcmp(a, b) == 0;
}

#include "generated_constants.inc"

static void print_hex_bytes(const unsigned char* bytes, long len) {
    for (long i = 0; i < len; i++) {
        printf("%02x", bytes[i]);
    }
}

static int load_file(const char* path, unsigned char** out, long* out_len) {
    FILE* fp = fopen(path, "rb");
    if (!fp) {
        return 1;
    }
    if (fseek(fp, 0, SEEK_END) != 0) {
        fclose(fp);
        return 1;
    }
    long len = ftell(fp);
    if (len < 0 || fseek(fp, 0, SEEK_SET) != 0) {
        fclose(fp);
        return 1;
    }
    unsigned char* data = (unsigned char*)malloc((size_t)len);
    if (!data) {
        fclose(fp);
        return 1;
    }
    if (fread(data, 1, (size_t)len, fp) != (size_t)len) {
        free(data);
        fclose(fp);
        return 1;
    }
    fclose(fp);
    *out = data;
    *out_len = len;
    return 0;
}

static unsigned char hex_nibble(char c) {
    if (c >= '0' && c <= '9') {
        return (unsigned char)(c - '0');
    }
    if (c >= 'a' && c <= 'f') {
        return (unsigned char)(10 + c - 'a');
    }
    if (c >= 'A' && c <= 'F') {
        return (unsigned char)(10 + c - 'A');
    }
    return 0;
}

static int decode_hex(const char* hex, unsigned char** out, long* out_len) {
    size_t chars = strlen(hex);
    if ((chars % 2) != 0) {
        return 1;
    }
    unsigned char* data = (unsigned char*)malloc(chars / 2);
    if (!data && chars > 0) {
        return 1;
    }
    for (size_t i = 0; i < chars; i += 2) {
        data[i / 2] = (unsigned char)((hex_nibble(hex[i]) << 4) | hex_nibble(hex[i + 1]));
    }
    *out = data;
    *out_len = (long)(chars / 2);
    return 0;
}

static void print_status(FT_Error err) {
    if (err == 0) {
        printf("\"status\":{\"kind\":\"ok\",\"error_code\":0}");
    } else {
        printf("\"status\":{\"kind\":\"error\",\"error_code\":%d}", err);
    }
}

static int emit_constant(const char* symbol) {
    long long value = 0;
    if (!emit_generated_constant_value(symbol, &value)) {
        fprintf(stderr, "unsupported constant: %s\n", symbol);
        return 2;
    }
    printf("{");
    print_status(0);
    printf(",\"output\":{\"value\":%lld}}\n", value);
    return 0;
}

#define FIELD(type, name) printf("{\"name\":\"%s\",\"offset\":%zu,\"size\":%zu}", #name, offsetof(type, name), sizeof(((type*)0)->name))

static int emit_layout(const char* record) {
    printf("{");
    print_status(0);
    printf(",\"output\":{");
    if (streq(record, "FT_Vector")) {
        printf("\"record\":\"FT_Vector\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Vector), _Alignof(FT_Vector));
        FIELD(FT_Vector, x);
        printf(",");
        FIELD(FT_Vector, y);
    } else if (streq(record, "FT_BBox")) {
        printf("\"record\":\"FT_BBox\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_BBox), _Alignof(FT_BBox));
        FIELD(FT_BBox, xMin);
        printf(",");
        FIELD(FT_BBox, yMin);
        printf(",");
        FIELD(FT_BBox, xMax);
        printf(",");
        FIELD(FT_BBox, yMax);
    } else if (streq(record, "FT_Glyph_Metrics")) {
        printf("\"record\":\"FT_Glyph_Metrics\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Glyph_Metrics), _Alignof(FT_Glyph_Metrics));
        FIELD(FT_Glyph_Metrics, width);
        printf(",");
        FIELD(FT_Glyph_Metrics, height);
        printf(",");
        FIELD(FT_Glyph_Metrics, horiBearingX);
        printf(",");
        FIELD(FT_Glyph_Metrics, horiBearingY);
        printf(",");
        FIELD(FT_Glyph_Metrics, horiAdvance);
        printf(",");
        FIELD(FT_Glyph_Metrics, vertBearingX);
        printf(",");
        FIELD(FT_Glyph_Metrics, vertBearingY);
        printf(",");
        FIELD(FT_Glyph_Metrics, vertAdvance);
    } else if (streq(record, "FT_Size_Metrics")) {
        printf("\"record\":\"FT_Size_Metrics\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Size_Metrics), _Alignof(FT_Size_Metrics));
        FIELD(FT_Size_Metrics, x_ppem);
        printf(",");
        FIELD(FT_Size_Metrics, y_ppem);
        printf(",");
        FIELD(FT_Size_Metrics, x_scale);
        printf(",");
        FIELD(FT_Size_Metrics, y_scale);
        printf(",");
        FIELD(FT_Size_Metrics, ascender);
        printf(",");
        FIELD(FT_Size_Metrics, descender);
        printf(",");
        FIELD(FT_Size_Metrics, height);
        printf(",");
        FIELD(FT_Size_Metrics, max_advance);
    } else if (streq(record, "FT_GlyphRec")) {
        printf("\"record\":\"FT_GlyphRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_GlyphRec), _Alignof(FT_GlyphRec));
        FIELD(FT_GlyphRec, library); printf(",");
        FIELD(FT_GlyphRec, clazz); printf(",");
        FIELD(FT_GlyphRec, format); printf(",");
        FIELD(FT_GlyphRec, advance);
    } else if (streq(record, "FT_BitmapGlyphRec")) {
        printf("\"record\":\"FT_BitmapGlyphRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_BitmapGlyphRec), _Alignof(FT_BitmapGlyphRec));
        FIELD(FT_BitmapGlyphRec, root); printf(",");
        FIELD(FT_BitmapGlyphRec, left); printf(",");
        FIELD(FT_BitmapGlyphRec, top); printf(",");
        FIELD(FT_BitmapGlyphRec, bitmap);
    } else if (streq(record, "FT_OutlineGlyphRec")) {
        printf("\"record\":\"FT_OutlineGlyphRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_OutlineGlyphRec), _Alignof(FT_OutlineGlyphRec));
        FIELD(FT_OutlineGlyphRec, root); printf(",");
        FIELD(FT_OutlineGlyphRec, outline);
    } else if (streq(record, "FT_SvgGlyphRec")) {
        printf("\"record\":\"FT_SvgGlyphRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_SvgGlyphRec), _Alignof(FT_SvgGlyphRec));
        FIELD(FT_SvgGlyphRec, root); printf(",");
        FIELD(FT_SvgGlyphRec, svg_document); printf(",");
        FIELD(FT_SvgGlyphRec, svg_document_length); printf(",");
        FIELD(FT_SvgGlyphRec, glyph_index); printf(",");
        FIELD(FT_SvgGlyphRec, metrics); printf(",");
        FIELD(FT_SvgGlyphRec, units_per_EM); printf(",");
        FIELD(FT_SvgGlyphRec, start_glyph_id); printf(",");
        FIELD(FT_SvgGlyphRec, end_glyph_id); printf(",");
        FIELD(FT_SvgGlyphRec, transform); printf(",");
        FIELD(FT_SvgGlyphRec, delta);
    } else if (streq(record, "FT_StreamRec")) {
        printf("\"record\":\"FT_StreamRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_StreamRec), _Alignof(FT_StreamRec));
        FIELD(FT_StreamRec, base); printf(",");
        FIELD(FT_StreamRec, size); printf(",");
        FIELD(FT_StreamRec, pos); printf(",");
        FIELD(FT_StreamRec, descriptor); printf(",");
        FIELD(FT_StreamRec, pathname); printf(",");
        FIELD(FT_StreamRec, read); printf(",");
        FIELD(FT_StreamRec, close); printf(",");
        FIELD(FT_StreamRec, memory); printf(",");
        FIELD(FT_StreamRec, cursor); printf(",");
        FIELD(FT_StreamRec, limit);
    } else if (streq(record, "FT_Bitmap_Size")) {
        printf("\"record\":\"FT_Bitmap_Size\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Bitmap_Size), _Alignof(FT_Bitmap_Size));
        FIELD(FT_Bitmap_Size, height); printf(",");
        FIELD(FT_Bitmap_Size, width); printf(",");
        FIELD(FT_Bitmap_Size, size); printf(",");
        FIELD(FT_Bitmap_Size, x_ppem); printf(",");
        FIELD(FT_Bitmap_Size, y_ppem);
    } else if (streq(record, "FT_Bitmap")) {
        printf("\"record\":\"FT_Bitmap\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Bitmap), _Alignof(FT_Bitmap));
        FIELD(FT_Bitmap, rows); printf(",");
        FIELD(FT_Bitmap, width); printf(",");
        FIELD(FT_Bitmap, pitch); printf(",");
        FIELD(FT_Bitmap, buffer); printf(",");
        FIELD(FT_Bitmap, num_grays); printf(",");
        FIELD(FT_Bitmap, pixel_mode); printf(",");
        FIELD(FT_Bitmap, palette_mode); printf(",");
        FIELD(FT_Bitmap, palette);
    } else if (streq(record, "FT_CharMapRec")) {
        printf("\"record\":\"FT_CharMapRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_CharMapRec), _Alignof(FT_CharMapRec));
        FIELD(FT_CharMapRec, face); printf(",");
        FIELD(FT_CharMapRec, encoding); printf(",");
        FIELD(FT_CharMapRec, platform_id); printf(",");
        FIELD(FT_CharMapRec, encoding_id);
    } else if (streq(record, "FT_SizeRec")) {
        printf("\"record\":\"FT_SizeRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_SizeRec), _Alignof(FT_SizeRec));
        FIELD(FT_SizeRec, face); printf(",");
        FIELD(FT_SizeRec, generic); printf(",");
        FIELD(FT_SizeRec, metrics); printf(",");
        FIELD(FT_SizeRec, internal);
    } else if (streq(record, "FT_FaceRec")) {
        printf("\"record\":\"FT_FaceRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_FaceRec), _Alignof(FT_FaceRec));
        FIELD(FT_FaceRec, num_faces); printf(",");
        FIELD(FT_FaceRec, face_index); printf(",");
        FIELD(FT_FaceRec, face_flags); printf(",");
        FIELD(FT_FaceRec, style_flags); printf(",");
        FIELD(FT_FaceRec, num_glyphs); printf(",");
        FIELD(FT_FaceRec, family_name); printf(",");
        FIELD(FT_FaceRec, style_name); printf(",");
        FIELD(FT_FaceRec, num_fixed_sizes); printf(",");
        FIELD(FT_FaceRec, available_sizes); printf(",");
        FIELD(FT_FaceRec, num_charmaps); printf(",");
        FIELD(FT_FaceRec, charmaps); printf(",");
        FIELD(FT_FaceRec, generic); printf(",");
        FIELD(FT_FaceRec, bbox); printf(",");
        FIELD(FT_FaceRec, units_per_EM); printf(",");
        FIELD(FT_FaceRec, ascender); printf(",");
        FIELD(FT_FaceRec, descender); printf(",");
        FIELD(FT_FaceRec, height); printf(",");
        FIELD(FT_FaceRec, max_advance_width); printf(",");
        FIELD(FT_FaceRec, max_advance_height); printf(",");
        FIELD(FT_FaceRec, underline_position); printf(",");
        FIELD(FT_FaceRec, underline_thickness); printf(",");
        FIELD(FT_FaceRec, glyph); printf(",");
        FIELD(FT_FaceRec, size); printf(",");
        FIELD(FT_FaceRec, charmap); printf(",");
        FIELD(FT_FaceRec, driver); printf(",");
        FIELD(FT_FaceRec, memory); printf(",");
        FIELD(FT_FaceRec, stream); printf(",");
        FIELD(FT_FaceRec, sizes_list); printf(",");
        FIELD(FT_FaceRec, autohint); printf(",");
        FIELD(FT_FaceRec, extensions); printf(",");
        FIELD(FT_FaceRec, internal);
    } else if (streq(record, "FT_GlyphSlotRec")) {
        printf("\"record\":\"FT_GlyphSlotRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_GlyphSlotRec), _Alignof(FT_GlyphSlotRec));
        FIELD(FT_GlyphSlotRec, library); printf(",");
        FIELD(FT_GlyphSlotRec, face); printf(",");
        FIELD(FT_GlyphSlotRec, next); printf(",");
        FIELD(FT_GlyphSlotRec, glyph_index); printf(",");
        FIELD(FT_GlyphSlotRec, generic); printf(",");
        FIELD(FT_GlyphSlotRec, metrics); printf(",");
        FIELD(FT_GlyphSlotRec, linearHoriAdvance); printf(",");
        FIELD(FT_GlyphSlotRec, linearVertAdvance); printf(",");
        FIELD(FT_GlyphSlotRec, advance); printf(",");
        FIELD(FT_GlyphSlotRec, format); printf(",");
        FIELD(FT_GlyphSlotRec, bitmap); printf(",");
        FIELD(FT_GlyphSlotRec, bitmap_left); printf(",");
        FIELD(FT_GlyphSlotRec, bitmap_top); printf(",");
        FIELD(FT_GlyphSlotRec, outline); printf(",");
        FIELD(FT_GlyphSlotRec, num_subglyphs); printf(",");
        FIELD(FT_GlyphSlotRec, subglyphs); printf(",");
        FIELD(FT_GlyphSlotRec, control_data); printf(",");
        FIELD(FT_GlyphSlotRec, control_len); printf(",");
        FIELD(FT_GlyphSlotRec, lsb_delta); printf(",");
        FIELD(FT_GlyphSlotRec, rsb_delta); printf(",");
        FIELD(FT_GlyphSlotRec, other); printf(",");
        FIELD(FT_GlyphSlotRec, internal);
    } else if (streq(record, "FT_Parameter")) {
        printf("\"record\":\"FT_Parameter\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Parameter), _Alignof(FT_Parameter));
        FIELD(FT_Parameter, tag); printf(",");
        FIELD(FT_Parameter, data);
    } else if (streq(record, "FT_Open_Args")) {
        printf("\"record\":\"FT_Open_Args\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Open_Args), _Alignof(FT_Open_Args));
        FIELD(FT_Open_Args, flags); printf(",");
        FIELD(FT_Open_Args, memory_base); printf(",");
        FIELD(FT_Open_Args, memory_size); printf(",");
        FIELD(FT_Open_Args, pathname); printf(",");
        FIELD(FT_Open_Args, stream); printf(",");
        FIELD(FT_Open_Args, driver); printf(",");
        FIELD(FT_Open_Args, num_params); printf(",");
        FIELD(FT_Open_Args, params);
    } else if (streq(record, "FT_Size_RequestRec")) {
        printf("\"record\":\"FT_Size_RequestRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Size_RequestRec), _Alignof(FT_Size_RequestRec));
        FIELD(FT_Size_RequestRec, type); printf(",");
        FIELD(FT_Size_RequestRec, width); printf(",");
        FIELD(FT_Size_RequestRec, height); printf(",");
        FIELD(FT_Size_RequestRec, horiResolution); printf(",");
        FIELD(FT_Size_RequestRec, vertResolution);
    } else if (streq(record, "FT_UnitVector")) {
        printf("\"record\":\"FT_UnitVector\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_UnitVector), _Alignof(FT_UnitVector));
        FIELD(FT_UnitVector, x); printf(",");
        FIELD(FT_UnitVector, y);
    } else if (streq(record, "FT_Matrix")) {
        printf("\"record\":\"FT_Matrix\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Matrix), _Alignof(FT_Matrix));
        FIELD(FT_Matrix, xx); printf(",");
        FIELD(FT_Matrix, xy); printf(",");
        FIELD(FT_Matrix, yx); printf(",");
        FIELD(FT_Matrix, yy);
    } else if (streq(record, "FT_Data")) {
        printf("\"record\":\"FT_Data\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Data), _Alignof(FT_Data));
        FIELD(FT_Data, pointer); printf(",");
        FIELD(FT_Data, length);
    } else if (streq(record, "FT_Generic")) {
        printf("\"record\":\"FT_Generic\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Generic), _Alignof(FT_Generic));
        FIELD(FT_Generic, data); printf(",");
        FIELD(FT_Generic, finalizer);
    } else if (streq(record, "FT_ListNodeRec")) {
        printf("\"record\":\"FT_ListNodeRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_ListNodeRec), _Alignof(FT_ListNodeRec));
        FIELD(FT_ListNodeRec, prev); printf(",");
        FIELD(FT_ListNodeRec, next); printf(",");
        FIELD(FT_ListNodeRec, data);
    } else if (streq(record, "FT_ListRec")) {
        printf("\"record\":\"FT_ListRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_ListRec), _Alignof(FT_ListRec));
        FIELD(FT_ListRec, head); printf(",");
        FIELD(FT_ListRec, tail);
    } else if (streq(record, "FT_Outline")) {
        printf("\"record\":\"FT_Outline\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Outline), _Alignof(FT_Outline));
        FIELD(FT_Outline, n_contours); printf(",");
        FIELD(FT_Outline, n_points); printf(",");
        FIELD(FT_Outline, points); printf(",");
        FIELD(FT_Outline, tags); printf(",");
        FIELD(FT_Outline, contours); printf(",");
        FIELD(FT_Outline, flags);
    } else if (streq(record, "FTC_ScalerRec")) {
        printf("\"record\":\"FTC_ScalerRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FTC_ScalerRec), _Alignof(FTC_ScalerRec));
        FIELD(FTC_ScalerRec, face_id); printf(",");
        FIELD(FTC_ScalerRec, width); printf(",");
        FIELD(FTC_ScalerRec, height); printf(",");
        FIELD(FTC_ScalerRec, pixel); printf(",");
        FIELD(FTC_ScalerRec, x_res); printf(",");
        FIELD(FTC_ScalerRec, y_res);
    } else if (streq(record, "FTC_ImageTypeRec")) {
        printf("\"record\":\"FTC_ImageTypeRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FTC_ImageTypeRec), _Alignof(FTC_ImageTypeRec));
        FIELD(FTC_ImageTypeRec, face_id); printf(",");
        FIELD(FTC_ImageTypeRec, width); printf(",");
        FIELD(FTC_ImageTypeRec, height); printf(",");
        FIELD(FTC_ImageTypeRec, flags);
    } else if (streq(record, "FTC_SBitRec")) {
        printf("\"record\":\"FTC_SBitRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FTC_SBitRec), _Alignof(FTC_SBitRec));
        FIELD(FTC_SBitRec, width); printf(",");
        FIELD(FTC_SBitRec, height); printf(",");
        FIELD(FTC_SBitRec, left); printf(",");
        FIELD(FTC_SBitRec, top); printf(",");
        FIELD(FTC_SBitRec, format); printf(",");
        FIELD(FTC_SBitRec, max_grays); printf(",");
        FIELD(FTC_SBitRec, pitch); printf(",");
        FIELD(FTC_SBitRec, xadvance); printf(",");
        FIELD(FTC_SBitRec, yadvance); printf(",");
        FIELD(FTC_SBitRec, buffer);
    } else if (streq(record, "FT_Color")) {
        printf("\"record\":\"FT_Color\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Color), _Alignof(FT_Color));
        FIELD(FT_Color, blue); printf(",");
        FIELD(FT_Color, green); printf(",");
        FIELD(FT_Color, red); printf(",");
        FIELD(FT_Color, alpha);
    } else if (streq(record, "FT_Palette_Data")) {
        printf("\"record\":\"FT_Palette_Data\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Palette_Data), _Alignof(FT_Palette_Data));
        FIELD(FT_Palette_Data, num_palettes); printf(",");
        FIELD(FT_Palette_Data, palette_name_ids); printf(",");
        FIELD(FT_Palette_Data, palette_flags); printf(",");
        FIELD(FT_Palette_Data, num_palette_entries); printf(",");
        FIELD(FT_Palette_Data, palette_entry_name_ids);
    } else if (streq(record, "FT_LayerIterator")) {
        printf("\"record\":\"FT_LayerIterator\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_LayerIterator), _Alignof(FT_LayerIterator));
        FIELD(FT_LayerIterator, num_layers); printf(",");
        FIELD(FT_LayerIterator, layer); printf(",");
        FIELD(FT_LayerIterator, p);
    } else if (streq(record, "FT_OpaquePaint")) {
        printf("\"record\":\"FT_OpaquePaint\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_OpaquePaint), _Alignof(FT_OpaquePaint));
        FIELD(FT_OpaquePaint, p); printf(",");
        FIELD(FT_OpaquePaint, insert_root_transform);
    } else if (streq(record, "FT_ColorStopIterator")) {
        printf("\"record\":\"FT_ColorStopIterator\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_ColorStopIterator), _Alignof(FT_ColorStopIterator));
        FIELD(FT_ColorStopIterator, num_color_stops); printf(",");
        FIELD(FT_ColorStopIterator, current_color_stop); printf(",");
        FIELD(FT_ColorStopIterator, p); printf(",");
        FIELD(FT_ColorStopIterator, read_variable);
    } else if (streq(record, "FT_ColorIndex")) {
        printf("\"record\":\"FT_ColorIndex\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_ColorIndex), _Alignof(FT_ColorIndex));
        FIELD(FT_ColorIndex, palette_index); printf(",");
        FIELD(FT_ColorIndex, alpha);
    } else if (streq(record, "FT_ColorStop")) {
        printf("\"record\":\"FT_ColorStop\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_ColorStop), _Alignof(FT_ColorStop));
        FIELD(FT_ColorStop, stop_offset); printf(",");
        FIELD(FT_ColorStop, color);
    } else if (streq(record, "FT_ColorLine")) {
        printf("\"record\":\"FT_ColorLine\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_ColorLine), _Alignof(FT_ColorLine));
        FIELD(FT_ColorLine, extend); printf(",");
        FIELD(FT_ColorLine, color_stop_iterator);
    } else if (streq(record, "FT_Affine23")) {
        printf("\"record\":\"FT_Affine23\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Affine23), _Alignof(FT_Affine23));
        FIELD(FT_Affine23, xx); printf(",");
        FIELD(FT_Affine23, xy); printf(",");
        FIELD(FT_Affine23, dx); printf(",");
        FIELD(FT_Affine23, yx); printf(",");
        FIELD(FT_Affine23, yy); printf(",");
        FIELD(FT_Affine23, dy);
    } else if (streq(record, "FT_PaintColrLayers")) {
        printf("\"record\":\"FT_PaintColrLayers\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintColrLayers), _Alignof(FT_PaintColrLayers));
        FIELD(FT_PaintColrLayers, layer_iterator);
    } else if (streq(record, "FT_PaintSolid")) {
        printf("\"record\":\"FT_PaintSolid\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintSolid), _Alignof(FT_PaintSolid));
        FIELD(FT_PaintSolid, color);
    } else if (streq(record, "FT_PaintLinearGradient")) {
        printf("\"record\":\"FT_PaintLinearGradient\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintLinearGradient), _Alignof(FT_PaintLinearGradient));
        FIELD(FT_PaintLinearGradient, colorline); printf(",");
        FIELD(FT_PaintLinearGradient, p0); printf(",");
        FIELD(FT_PaintLinearGradient, p1); printf(",");
        FIELD(FT_PaintLinearGradient, p2);
    } else if (streq(record, "FT_PaintRadialGradient")) {
        printf("\"record\":\"FT_PaintRadialGradient\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintRadialGradient), _Alignof(FT_PaintRadialGradient));
        FIELD(FT_PaintRadialGradient, colorline); printf(",");
        FIELD(FT_PaintRadialGradient, c0); printf(",");
        FIELD(FT_PaintRadialGradient, r0); printf(",");
        FIELD(FT_PaintRadialGradient, c1); printf(",");
        FIELD(FT_PaintRadialGradient, r1);
    } else if (streq(record, "FT_PaintSweepGradient")) {
        printf("\"record\":\"FT_PaintSweepGradient\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintSweepGradient), _Alignof(FT_PaintSweepGradient));
        FIELD(FT_PaintSweepGradient, colorline); printf(",");
        FIELD(FT_PaintSweepGradient, center); printf(",");
        FIELD(FT_PaintSweepGradient, start_angle); printf(",");
        FIELD(FT_PaintSweepGradient, end_angle);
    } else if (streq(record, "FT_PaintGlyph")) {
        printf("\"record\":\"FT_PaintGlyph\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintGlyph), _Alignof(FT_PaintGlyph));
        FIELD(FT_PaintGlyph, paint); printf(",");
        FIELD(FT_PaintGlyph, glyphID);
    } else if (streq(record, "FT_PaintColrGlyph")) {
        printf("\"record\":\"FT_PaintColrGlyph\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintColrGlyph), _Alignof(FT_PaintColrGlyph));
        FIELD(FT_PaintColrGlyph, glyphID);
    } else if (streq(record, "FT_PaintTransform")) {
        printf("\"record\":\"FT_PaintTransform\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintTransform), _Alignof(FT_PaintTransform));
        FIELD(FT_PaintTransform, paint); printf(",");
        FIELD(FT_PaintTransform, affine);
    } else if (streq(record, "FT_PaintTranslate")) {
        printf("\"record\":\"FT_PaintTranslate\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintTranslate), _Alignof(FT_PaintTranslate));
        FIELD(FT_PaintTranslate, paint); printf(",");
        FIELD(FT_PaintTranslate, dx); printf(",");
        FIELD(FT_PaintTranslate, dy);
    } else if (streq(record, "FT_PaintScale")) {
        printf("\"record\":\"FT_PaintScale\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintScale), _Alignof(FT_PaintScale));
        FIELD(FT_PaintScale, paint); printf(",");
        FIELD(FT_PaintScale, scale_x); printf(",");
        FIELD(FT_PaintScale, scale_y); printf(",");
        FIELD(FT_PaintScale, center_x); printf(",");
        FIELD(FT_PaintScale, center_y);
    } else if (streq(record, "FT_PaintRotate")) {
        printf("\"record\":\"FT_PaintRotate\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintRotate), _Alignof(FT_PaintRotate));
        FIELD(FT_PaintRotate, paint); printf(",");
        FIELD(FT_PaintRotate, angle); printf(",");
        FIELD(FT_PaintRotate, center_x); printf(",");
        FIELD(FT_PaintRotate, center_y);
    } else if (streq(record, "FT_PaintSkew")) {
        printf("\"record\":\"FT_PaintSkew\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintSkew), _Alignof(FT_PaintSkew));
        FIELD(FT_PaintSkew, paint); printf(",");
        FIELD(FT_PaintSkew, x_skew_angle); printf(",");
        FIELD(FT_PaintSkew, y_skew_angle); printf(",");
        FIELD(FT_PaintSkew, center_x); printf(",");
        FIELD(FT_PaintSkew, center_y);
    } else if (streq(record, "FT_PaintComposite")) {
        printf("\"record\":\"FT_PaintComposite\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_PaintComposite), _Alignof(FT_PaintComposite));
        FIELD(FT_PaintComposite, source_paint); printf(",");
        FIELD(FT_PaintComposite, composite_mode); printf(",");
        FIELD(FT_PaintComposite, backdrop_paint);
    } else if (streq(record, "FT_ClipBox")) {
        printf("\"record\":\"FT_ClipBox\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_ClipBox), _Alignof(FT_ClipBox));
        FIELD(FT_ClipBox, bottom_left); printf(",");
        FIELD(FT_ClipBox, top_left); printf(",");
        FIELD(FT_ClipBox, top_right); printf(",");
        FIELD(FT_ClipBox, bottom_right);
    } else if (streq(record, "FT_Outline_Funcs")) {
        printf("\"record\":\"FT_Outline_Funcs\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Outline_Funcs), _Alignof(FT_Outline_Funcs));
        FIELD(FT_Outline_Funcs, move_to); printf(",");
        FIELD(FT_Outline_Funcs, line_to); printf(",");
        FIELD(FT_Outline_Funcs, conic_to); printf(",");
        FIELD(FT_Outline_Funcs, cubic_to); printf(",");
        FIELD(FT_Outline_Funcs, shift); printf(",");
        FIELD(FT_Outline_Funcs, delta);
    } else if (streq(record, "FT_Span")) {
        printf("\"record\":\"FT_Span\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Span), _Alignof(FT_Span));
        FIELD(FT_Span, x); printf(",");
        FIELD(FT_Span, len); printf(",");
        FIELD(FT_Span, coverage);
    } else if (streq(record, "FT_Raster_Params")) {
        printf("\"record\":\"FT_Raster_Params\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Raster_Params), _Alignof(FT_Raster_Params));
        FIELD(FT_Raster_Params, target); printf(",");
        FIELD(FT_Raster_Params, source); printf(",");
        FIELD(FT_Raster_Params, flags); printf(",");
        FIELD(FT_Raster_Params, gray_spans); printf(",");
        FIELD(FT_Raster_Params, black_spans); printf(",");
        FIELD(FT_Raster_Params, bit_test); printf(",");
        FIELD(FT_Raster_Params, bit_set); printf(",");
        FIELD(FT_Raster_Params, user); printf(",");
        FIELD(FT_Raster_Params, clip_box);
    } else if (streq(record, "FT_Raster_Funcs")) {
        printf("\"record\":\"FT_Raster_Funcs\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Raster_Funcs), _Alignof(FT_Raster_Funcs));
        FIELD(FT_Raster_Funcs, glyph_format); printf(",");
        FIELD(FT_Raster_Funcs, raster_new); printf(",");
        FIELD(FT_Raster_Funcs, raster_reset); printf(",");
        FIELD(FT_Raster_Funcs, raster_set_mode); printf(",");
        FIELD(FT_Raster_Funcs, raster_render); printf(",");
        FIELD(FT_Raster_Funcs, raster_done);
    } else if (streq(record, "FT_MM_Axis")) {
        printf("\"record\":\"FT_MM_Axis\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_MM_Axis), _Alignof(FT_MM_Axis));
        FIELD(FT_MM_Axis, name); printf(",");
        FIELD(FT_MM_Axis, minimum); printf(",");
        FIELD(FT_MM_Axis, maximum);
    } else if (streq(record, "FT_Multi_Master")) {
        printf("\"record\":\"FT_Multi_Master\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Multi_Master), _Alignof(FT_Multi_Master));
        FIELD(FT_Multi_Master, num_axis); printf(",");
        FIELD(FT_Multi_Master, num_designs); printf(",");
        FIELD(FT_Multi_Master, axis);
    } else if (streq(record, "FT_Var_Axis")) {
        printf("\"record\":\"FT_Var_Axis\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Var_Axis), _Alignof(FT_Var_Axis));
        FIELD(FT_Var_Axis, name); printf(",");
        FIELD(FT_Var_Axis, minimum); printf(",");
        FIELD(FT_Var_Axis, def); printf(",");
        FIELD(FT_Var_Axis, maximum); printf(",");
        FIELD(FT_Var_Axis, tag); printf(",");
        FIELD(FT_Var_Axis, strid);
    } else if (streq(record, "FT_Var_Named_Style")) {
        printf("\"record\":\"FT_Var_Named_Style\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Var_Named_Style), _Alignof(FT_Var_Named_Style));
        FIELD(FT_Var_Named_Style, coords); printf(",");
        FIELD(FT_Var_Named_Style, strid); printf(",");
        FIELD(FT_Var_Named_Style, psid);
    } else if (streq(record, "FT_MM_Var")) {
        printf("\"record\":\"FT_MM_Var\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_MM_Var), _Alignof(FT_MM_Var));
        FIELD(FT_MM_Var, num_axis); printf(",");
        FIELD(FT_MM_Var, num_designs); printf(",");
        FIELD(FT_MM_Var, num_namedstyles); printf(",");
        FIELD(FT_MM_Var, axis); printf(",");
        FIELD(FT_MM_Var, namedstyle);
    } else if (streq(record, "FT_Prop_GlyphToScriptMap")) {
        printf("\"record\":\"FT_Prop_GlyphToScriptMap\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Prop_GlyphToScriptMap), _Alignof(FT_Prop_GlyphToScriptMap));
        FIELD(FT_Prop_GlyphToScriptMap, face); printf(",");
        FIELD(FT_Prop_GlyphToScriptMap, map);
    } else if (streq(record, "FT_Prop_IncreaseXHeight")) {
        printf("\"record\":\"FT_Prop_IncreaseXHeight\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Prop_IncreaseXHeight), _Alignof(FT_Prop_IncreaseXHeight));
        FIELD(FT_Prop_IncreaseXHeight, face); printf(",");
        FIELD(FT_Prop_IncreaseXHeight, limit);
    } else if (streq(record, "FT_Incremental_MetricsRec")) {
        printf("\"record\":\"FT_Incremental_MetricsRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Incremental_MetricsRec), _Alignof(FT_Incremental_MetricsRec));
        FIELD(FT_Incremental_MetricsRec, bearing_x); printf(",");
        FIELD(FT_Incremental_MetricsRec, bearing_y); printf(",");
        FIELD(FT_Incremental_MetricsRec, advance); printf(",");
        FIELD(FT_Incremental_MetricsRec, advance_v);
    } else if (streq(record, "FT_Incremental_FuncsRec")) {
        printf("\"record\":\"FT_Incremental_FuncsRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Incremental_FuncsRec), _Alignof(FT_Incremental_FuncsRec));
        FIELD(FT_Incremental_FuncsRec, get_glyph_data); printf(",");
        FIELD(FT_Incremental_FuncsRec, free_glyph_data); printf(",");
        FIELD(FT_Incremental_FuncsRec, get_glyph_metrics);
    } else if (streq(record, "FT_Incremental_InterfaceRec")) {
        printf("\"record\":\"FT_Incremental_InterfaceRec\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Incremental_InterfaceRec), _Alignof(FT_Incremental_InterfaceRec));
        FIELD(FT_Incremental_InterfaceRec, funcs); printf(",");
        FIELD(FT_Incremental_InterfaceRec, object);
    } else if (streq(record, "FT_Module_Class")) {
        printf("\"record\":\"FT_Module_Class\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Module_Class), _Alignof(FT_Module_Class));
        FIELD(FT_Module_Class, module_flags); printf(",");
        FIELD(FT_Module_Class, module_size); printf(",");
        FIELD(FT_Module_Class, module_name); printf(",");
        FIELD(FT_Module_Class, module_version); printf(",");
        FIELD(FT_Module_Class, module_requires); printf(",");
        FIELD(FT_Module_Class, module_interface); printf(",");
        FIELD(FT_Module_Class, module_init); printf(",");
        FIELD(FT_Module_Class, module_done); printf(",");
        FIELD(FT_Module_Class, get_interface);
    } else if (streq(record, "FT_Renderer_Class")) {
        printf("\"record\":\"FT_Renderer_Class\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_Renderer_Class), _Alignof(FT_Renderer_Class));
        FIELD(FT_Renderer_Class, root); printf(",");
        FIELD(FT_Renderer_Class, glyph_format); printf(",");
        FIELD(FT_Renderer_Class, render_glyph); printf(",");
        FIELD(FT_Renderer_Class, transform_glyph); printf(",");
        FIELD(FT_Renderer_Class, get_glyph_cbox); printf(",");
        FIELD(FT_Renderer_Class, set_mode); printf(",");
        FIELD(FT_Renderer_Class, raster_class);
    } else if (streq(record, "FT_SfntName")) {
        printf("\"record\":\"FT_SfntName\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_SfntName), _Alignof(FT_SfntName));
        FIELD(FT_SfntName, platform_id); printf(",");
        FIELD(FT_SfntName, encoding_id); printf(",");
        FIELD(FT_SfntName, language_id); printf(",");
        FIELD(FT_SfntName, name_id); printf(",");
        FIELD(FT_SfntName, string); printf(",");
        FIELD(FT_SfntName, string_len);
    } else if (streq(record, "FT_SfntLangTag")) {
        printf("\"record\":\"FT_SfntLangTag\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(FT_SfntLangTag), _Alignof(FT_SfntLangTag));
        FIELD(FT_SfntLangTag, string); printf(",");
        FIELD(FT_SfntLangTag, string_len);
    } else if (streq(record, "T1_FontInfo")) {
        printf("\"record\":\"T1_FontInfo\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(T1_FontInfo), _Alignof(T1_FontInfo));
        FIELD(T1_FontInfo, version); printf(",");
        FIELD(T1_FontInfo, notice); printf(",");
        FIELD(T1_FontInfo, full_name); printf(",");
        FIELD(T1_FontInfo, family_name); printf(",");
        FIELD(T1_FontInfo, weight); printf(",");
        FIELD(T1_FontInfo, italic_angle); printf(",");
        FIELD(T1_FontInfo, is_fixed_pitch); printf(",");
        FIELD(T1_FontInfo, underline_position); printf(",");
        FIELD(T1_FontInfo, underline_thickness);
    } else if (streq(record, "T1_Private")) {
        printf("\"record\":\"T1_Private\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(T1_Private), _Alignof(T1_Private));
        FIELD(T1_Private, unique_id); printf(",");
        FIELD(T1_Private, lenIV); printf(",");
        FIELD(T1_Private, num_blue_values); printf(",");
        FIELD(T1_Private, num_other_blues); printf(",");
        FIELD(T1_Private, num_family_blues); printf(",");
        FIELD(T1_Private, num_family_other_blues); printf(",");
        FIELD(T1_Private, blue_values); printf(",");
        FIELD(T1_Private, other_blues); printf(",");
        FIELD(T1_Private, family_blues); printf(",");
        FIELD(T1_Private, family_other_blues); printf(",");
        FIELD(T1_Private, blue_scale); printf(",");
        FIELD(T1_Private, blue_shift); printf(",");
        FIELD(T1_Private, blue_fuzz); printf(",");
        FIELD(T1_Private, standard_width); printf(",");
        FIELD(T1_Private, standard_height); printf(",");
        FIELD(T1_Private, num_snap_widths); printf(",");
        FIELD(T1_Private, num_snap_heights); printf(",");
        FIELD(T1_Private, force_bold); printf(",");
        FIELD(T1_Private, round_stem_up); printf(",");
        FIELD(T1_Private, snap_widths); printf(",");
        FIELD(T1_Private, snap_heights); printf(",");
        FIELD(T1_Private, expansion_factor); printf(",");
        FIELD(T1_Private, language_group); printf(",");
        FIELD(T1_Private, password); printf(",");
        FIELD(T1_Private, min_feature);
    } else if (streq(record, "TT_Header")) {
        printf("\"record\":\"TT_Header\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(TT_Header), _Alignof(TT_Header));
        FIELD(TT_Header, Table_Version); printf(",");
        FIELD(TT_Header, Font_Revision); printf(",");
        FIELD(TT_Header, CheckSum_Adjust); printf(",");
        FIELD(TT_Header, Magic_Number); printf(",");
        FIELD(TT_Header, Flags); printf(",");
        FIELD(TT_Header, Units_Per_EM); printf(",");
        FIELD(TT_Header, Created); printf(",");
        FIELD(TT_Header, Modified); printf(",");
        FIELD(TT_Header, xMin); printf(",");
        FIELD(TT_Header, yMin); printf(",");
        FIELD(TT_Header, xMax); printf(",");
        FIELD(TT_Header, yMax); printf(",");
        FIELD(TT_Header, Mac_Style); printf(",");
        FIELD(TT_Header, Lowest_Rec_PPEM); printf(",");
        FIELD(TT_Header, Font_Direction); printf(",");
        FIELD(TT_Header, Index_To_Loc_Format); printf(",");
        FIELD(TT_Header, Glyph_Data_Format);
    } else if (streq(record, "TT_HoriHeader")) {
        printf("\"record\":\"TT_HoriHeader\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(TT_HoriHeader), _Alignof(TT_HoriHeader));
        FIELD(TT_HoriHeader, Version); printf(",");
        FIELD(TT_HoriHeader, Ascender); printf(",");
        FIELD(TT_HoriHeader, Descender); printf(",");
        FIELD(TT_HoriHeader, Line_Gap); printf(",");
        FIELD(TT_HoriHeader, advance_Width_Max); printf(",");
        FIELD(TT_HoriHeader, min_Left_Side_Bearing); printf(",");
        FIELD(TT_HoriHeader, min_Right_Side_Bearing); printf(",");
        FIELD(TT_HoriHeader, xMax_Extent); printf(",");
        FIELD(TT_HoriHeader, caret_Slope_Rise); printf(",");
        FIELD(TT_HoriHeader, caret_Slope_Run); printf(",");
        FIELD(TT_HoriHeader, caret_Offset); printf(",");
        FIELD(TT_HoriHeader, Reserved); printf(",");
        FIELD(TT_HoriHeader, metric_Data_Format); printf(",");
        FIELD(TT_HoriHeader, number_Of_HMetrics); printf(",");
        FIELD(TT_HoriHeader, long_metrics); printf(",");
        FIELD(TT_HoriHeader, short_metrics);
    } else if (streq(record, "TT_VertHeader")) {
        printf("\"record\":\"TT_VertHeader\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(TT_VertHeader), _Alignof(TT_VertHeader));
        FIELD(TT_VertHeader, Version); printf(",");
        FIELD(TT_VertHeader, Ascender); printf(",");
        FIELD(TT_VertHeader, Descender); printf(",");
        FIELD(TT_VertHeader, Line_Gap); printf(",");
        FIELD(TT_VertHeader, advance_Height_Max); printf(",");
        FIELD(TT_VertHeader, min_Top_Side_Bearing); printf(",");
        FIELD(TT_VertHeader, min_Bottom_Side_Bearing); printf(",");
        FIELD(TT_VertHeader, yMax_Extent); printf(",");
        FIELD(TT_VertHeader, caret_Slope_Rise); printf(",");
        FIELD(TT_VertHeader, caret_Slope_Run); printf(",");
        FIELD(TT_VertHeader, caret_Offset); printf(",");
        FIELD(TT_VertHeader, Reserved); printf(",");
        FIELD(TT_VertHeader, metric_Data_Format); printf(",");
        FIELD(TT_VertHeader, number_Of_VMetrics); printf(",");
        FIELD(TT_VertHeader, long_metrics); printf(",");
        FIELD(TT_VertHeader, short_metrics);
    } else if (streq(record, "TT_OS2")) {
        printf("\"record\":\"TT_OS2\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(TT_OS2), _Alignof(TT_OS2));
        FIELD(TT_OS2, version); printf(",");
        FIELD(TT_OS2, xAvgCharWidth); printf(",");
        FIELD(TT_OS2, usWeightClass); printf(",");
        FIELD(TT_OS2, usWidthClass); printf(",");
        FIELD(TT_OS2, fsType); printf(",");
        FIELD(TT_OS2, ySubscriptXSize); printf(",");
        FIELD(TT_OS2, ySubscriptYSize); printf(",");
        FIELD(TT_OS2, ySubscriptXOffset); printf(",");
        FIELD(TT_OS2, ySubscriptYOffset); printf(",");
        FIELD(TT_OS2, ySuperscriptXSize); printf(",");
        FIELD(TT_OS2, ySuperscriptYSize); printf(",");
        FIELD(TT_OS2, ySuperscriptXOffset); printf(",");
        FIELD(TT_OS2, ySuperscriptYOffset); printf(",");
        FIELD(TT_OS2, yStrikeoutSize); printf(",");
        FIELD(TT_OS2, yStrikeoutPosition); printf(",");
        FIELD(TT_OS2, sFamilyClass); printf(",");
        FIELD(TT_OS2, panose); printf(",");
        FIELD(TT_OS2, ulUnicodeRange1); printf(",");
        FIELD(TT_OS2, ulUnicodeRange2); printf(",");
        FIELD(TT_OS2, ulUnicodeRange3); printf(",");
        FIELD(TT_OS2, ulUnicodeRange4); printf(",");
        FIELD(TT_OS2, achVendID); printf(",");
        FIELD(TT_OS2, fsSelection); printf(",");
        FIELD(TT_OS2, usFirstCharIndex); printf(",");
        FIELD(TT_OS2, usLastCharIndex); printf(",");
        FIELD(TT_OS2, sTypoAscender); printf(",");
        FIELD(TT_OS2, sTypoDescender); printf(",");
        FIELD(TT_OS2, sTypoLineGap); printf(",");
        FIELD(TT_OS2, usWinAscent); printf(",");
        FIELD(TT_OS2, usWinDescent); printf(",");
        FIELD(TT_OS2, ulCodePageRange1); printf(",");
        FIELD(TT_OS2, ulCodePageRange2); printf(",");
        FIELD(TT_OS2, sxHeight); printf(",");
        FIELD(TT_OS2, sCapHeight); printf(",");
        FIELD(TT_OS2, usDefaultChar); printf(",");
        FIELD(TT_OS2, usBreakChar); printf(",");
        FIELD(TT_OS2, usMaxContext); printf(",");
        FIELD(TT_OS2, usLowerOpticalPointSize); printf(",");
        FIELD(TT_OS2, usUpperOpticalPointSize);
    } else if (streq(record, "TT_Postscript")) {
        printf("\"record\":\"TT_Postscript\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(TT_Postscript), _Alignof(TT_Postscript));
        FIELD(TT_Postscript, FormatType); printf(",");
        FIELD(TT_Postscript, italicAngle); printf(",");
        FIELD(TT_Postscript, underlinePosition); printf(",");
        FIELD(TT_Postscript, underlineThickness); printf(",");
        FIELD(TT_Postscript, isFixedPitch); printf(",");
        FIELD(TT_Postscript, minMemType42); printf(",");
        FIELD(TT_Postscript, maxMemType42); printf(",");
        FIELD(TT_Postscript, minMemType1); printf(",");
        FIELD(TT_Postscript, maxMemType1);
    } else if (streq(record, "TT_PCLT")) {
        printf("\"record\":\"TT_PCLT\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(TT_PCLT), _Alignof(TT_PCLT));
        FIELD(TT_PCLT, Version); printf(",");
        FIELD(TT_PCLT, FontNumber); printf(",");
        FIELD(TT_PCLT, Pitch); printf(",");
        FIELD(TT_PCLT, xHeight); printf(",");
        FIELD(TT_PCLT, Style); printf(",");
        FIELD(TT_PCLT, TypeFamily); printf(",");
        FIELD(TT_PCLT, CapHeight); printf(",");
        FIELD(TT_PCLT, SymbolSet); printf(",");
        FIELD(TT_PCLT, TypeFace); printf(",");
        FIELD(TT_PCLT, CharacterComplement); printf(",");
        FIELD(TT_PCLT, FileName); printf(",");
        FIELD(TT_PCLT, StrokeWeight); printf(",");
        FIELD(TT_PCLT, WidthType); printf(",");
        FIELD(TT_PCLT, SerifStyle); printf(",");
        FIELD(TT_PCLT, Reserved);
    } else if (streq(record, "TT_MaxProfile")) {
        printf("\"record\":\"TT_MaxProfile\",\"size\":%zu,\"align\":%zu,\"fields\":[", sizeof(TT_MaxProfile), _Alignof(TT_MaxProfile));
        FIELD(TT_MaxProfile, version); printf(",");
        FIELD(TT_MaxProfile, numGlyphs); printf(",");
        FIELD(TT_MaxProfile, maxPoints); printf(",");
        FIELD(TT_MaxProfile, maxContours); printf(",");
        FIELD(TT_MaxProfile, maxCompositePoints); printf(",");
        FIELD(TT_MaxProfile, maxCompositeContours); printf(",");
        FIELD(TT_MaxProfile, maxZones); printf(",");
        FIELD(TT_MaxProfile, maxTwilightPoints); printf(",");
        FIELD(TT_MaxProfile, maxStorage); printf(",");
        FIELD(TT_MaxProfile, maxFunctionDefs); printf(",");
        FIELD(TT_MaxProfile, maxInstructionDefs); printf(",");
        FIELD(TT_MaxProfile, maxStackElements); printf(",");
        FIELD(TT_MaxProfile, maxSizeOfInstructions); printf(",");
        FIELD(TT_MaxProfile, maxComponentElements); printf(",");
        FIELD(TT_MaxProfile, maxComponentDepth);
    } else {
        fprintf(stderr, "unsupported record: %s\n", record);
        return 2;
    }
    printf("]}}\n");
    return 0;
}

#define EMIT_SCALAR_TYPE(name) \
    if (streq(symbol, #name)) { \
        printf("{"); \
        print_status(0); \
        printf(",\"output\":{\"symbol\":\"%s\",\"kind\":\"scalar\",\"size\":%zu,\"align\":%zu,\"signed\":%s}}\n", \
               #name, sizeof(name), _Alignof(name), (((name)-1) < (name)0) ? "true" : "false"); \
        return 0; \
    } else

#define EMIT_POINTER_TYPE(name) \
    if (streq(symbol, #name)) { \
        printf("{"); \
        print_status(0); \
        printf(",\"output\":{\"symbol\":\"%s\",\"kind\":\"pointer\",\"size\":%zu,\"align\":%zu,\"signed\":null}}\n", \
               #name, sizeof(name), _Alignof(name)); \
        return 0; \
    } else

static int emit_type_probe(const char* symbol) {
    EMIT_SCALAR_TYPE(FT_Offset)
    EMIT_SCALAR_TYPE(FT_UFWord)
    EMIT_SCALAR_TYPE(FT_F2Dot14)
    EMIT_SCALAR_TYPE(FT_UInt)
    EMIT_SCALAR_TYPE(FT_Error)
    EMIT_SCALAR_TYPE(FT_ULong)
    EMIT_SCALAR_TYPE(FT_Char)
    EMIT_SCALAR_TYPE(FT_Int)
    EMIT_SCALAR_TYPE(FT_Short)
    EMIT_SCALAR_TYPE(FT_Tag)
    EMIT_SCALAR_TYPE(FT_String)
    EMIT_SCALAR_TYPE(FT_Long)
    EMIT_SCALAR_TYPE(FT_PtrDist)
    EMIT_SCALAR_TYPE(FT_FWord)
    EMIT_SCALAR_TYPE(FT_Fixed)
    EMIT_SCALAR_TYPE(FT_F26Dot6)
    EMIT_SCALAR_TYPE(FT_UShort)
    EMIT_SCALAR_TYPE(FT_Pos)
    EMIT_SCALAR_TYPE(FT_Sfnt_Tag)
    EMIT_POINTER_TYPE(FT_Bytes)
    EMIT_POINTER_TYPE(FT_ListNode)
    EMIT_POINTER_TYPE(FT_Pointer)
    EMIT_POINTER_TYPE(FT_List)
    EMIT_POINTER_TYPE(FT_Size)
    EMIT_POINTER_TYPE(FT_Renderer)
    EMIT_POINTER_TYPE(FT_Stream)
    EMIT_POINTER_TYPE(FT_Size_Internal)
    EMIT_POINTER_TYPE(FTC_Scaler)
    EMIT_POINTER_TYPE(FTC_ImageType)
    EMIT_POINTER_TYPE(FTC_Node)
    EMIT_POINTER_TYPE(FT_Module)
    EMIT_POINTER_TYPE(FT_Slot_Internal)
    EMIT_POINTER_TYPE(FT_Face_Internal)
    EMIT_POINTER_TYPE(FT_CharMap)
    EMIT_POINTER_TYPE(FT_Memory)
    EMIT_POINTER_TYPE(FTC_FaceID)
    EMIT_POINTER_TYPE(FT_SubGlyph)
    EMIT_POINTER_TYPE(FTC_SBit)
    EMIT_POINTER_TYPE(FTC_Manager)
    EMIT_POINTER_TYPE(FTC_CMapCache)
    EMIT_POINTER_TYPE(FT_Driver)
    EMIT_POINTER_TYPE(FTC_ImageCache)
    EMIT_POINTER_TYPE(FTC_SBitCache)
    EMIT_POINTER_TYPE(FT_Raster)
    {
        fprintf(stderr, "unsupported type probe: %s\n", symbol);
        return 2;
    }
}

#undef EMIT_SCALAR_TYPE
#undef EMIT_POINTER_TYPE

static int emit_function_probe(const char* symbol) {
#define EMIT_FUNCTION(name) \
    if (streq(symbol, #name)) { \
        (void)&name; \
        printf("{"); \
        print_status(0); \
        printf(",\"output\":{\"symbol\":\"%s\",\"kind\":\"function\"}}\n", #name); \
        return 0; \
    } else

    EMIT_FUNCTION(FT_Get_CMap_Format)
    EMIT_FUNCTION(FT_Get_CMap_Language_ID)
    EMIT_FUNCTION(FT_Get_Sfnt_Table)
    EMIT_FUNCTION(FT_Load_Sfnt_Table)
    EMIT_FUNCTION(FT_Sfnt_Table_Info)
    {
        fprintf(stderr, "unsupported function probe: %s\n", symbol);
        return 2;
    }

#undef EMIT_FUNCTION
}

static void print_json_bool(int value) {
    printf(value ? "true" : "false");
}

static void print_ok_output_prefix(void) {
    printf("{");
    print_status(0);
    printf(",\"output\":");
}

static int emit_macro_import_contract(const char* expansion) {
    print_ok_output_prefix();
    printf("{\"macro_defined\":true,\"expansion_model\":\"%s\"}}\n", expansion);
    return 0;
}

static void print_tag_row(const char* label, unsigned long tag) {
    printf("{\"label\":\"%s\",\"tag\":%lu,\"hex\":\"0x%08lx\"}", label, tag, tag);
}

static void print_error_base_row(long error) {
    printf("{\"error\":%ld,\"base\":%ld}", error, (long)FT_ERROR_BASE(error));
}

static void print_error_module_row(long error) {
    printf("{\"error\":%ld,\"module\":%lu}", error, (unsigned long)FT_ERROR_MODULE(error));
}

static void print_error_pair_row(long x, const char* e, int result) {
    printf("{\"x\":%ld,\"e\":\"%s\",\"result\":", x, e);
    print_json_bool(result);
    printf("}");
}

static void print_bool_input_row(const char* input, FT_Bool result) {
    printf("{\"input\":\"%s\",\"result\":%u}", input, (unsigned int)result);
}

static int emit_macro_eval(const char* case_id) {
    if (streq(case_id, "fttypes.FT_BOOL.macro_import_contract")) {
        return emit_macro_import_contract("FT_STATIC_CAST(FT_Bool, (x) != 0)");
    }
    if (streq(case_id, "fttypes.FT_BOOL.zero_maps_to_false")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_bool_input_row("0", FT_BOOL(0));
        printf(",");
        print_bool_input_row("0L", FT_BOOL(0L));
        printf(",");
        print_bool_input_row("(void*)0", FT_BOOL((void*)0));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_BOOL.any_nonzero_maps_to_true")) {
        int sentinel = 1;
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_bool_input_row("1", FT_BOOL(1));
        printf(",");
        print_bool_input_row("2", FT_BOOL(2));
        printf(",");
        print_bool_input_row("-1", FT_BOOL(-1));
        printf(",");
        print_bool_input_row("255", FT_BOOL(255));
        printf(",");
        print_bool_input_row("pointer_token", FT_BOOL(&sentinel));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_BOOL.result_type_is_ft_bool")) {
        print_ok_output_prefix();
        printf("{\"sizeof_result\":%zu,\"alignof_result\":%zu,\"value_storage\":\"unsigned char\"}}\n",
               sizeof(FT_BOOL(2)), _Alignof(FT_Bool));
        return 0;
    }

    if (streq(case_id, "fttypes.FT_ERROR_BASE.macro_import_contract")) {
        return emit_macro_import_contract("((x) & 0xFF)");
    }
    if (streq(case_id, "fttypes.FT_ERROR_BASE.base_byte_extraction")) {
        long values[] = {1, 2, 7, 263, 4615, FT_Mod_Err_Base | FT_Err_Invalid_Argument};
        print_ok_output_prefix();
        printf("{\"rows\":[");
        for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
            if (i) printf(",");
            print_error_base_row(values[i]);
        }
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERROR_BASE.zero_and_full_mask_edges")) {
        long values[] = {0, 255, 256, 512, 65280, 65535, -1};
        print_ok_output_prefix();
        printf("{\"rows\":[");
        for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
            if (i) printf(",");
            print_error_base_row(values[i]);
        }
        printf("]}}\n");
        return 0;
    }

    if (streq(case_id, "fttypes.FT_ERROR_MODULE.macro_import_contract")) {
        return emit_macro_import_contract("((x) & 0xFF00U)");
    }
    if (streq(case_id, "fttypes.FT_ERROR_MODULE.module_byte_extraction")) {
        long values[] = {256, 512, 4096, 4615, FT_Mod_Err_Base | FT_Err_Invalid_Argument};
        print_ok_output_prefix();
        printf("{\"rows\":[");
        for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
            if (i) printf(",");
            print_error_module_row(values[i]);
        }
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERROR_MODULE.zero_and_mixed_value_edges")) {
        long values[] = {0, 255, 256, 257, 4615, 65280, 65535, -1};
        print_ok_output_prefix();
        printf("{\"rows\":[");
        for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
            if (i) printf(",");
            print_error_module_row(values[i]);
        }
        printf("]}}\n");
        return 0;
    }

    if (streq(case_id, "fttypes.FT_ERR.macro_import_contract")) {
        return emit_macro_import_contract("FT_ERR_CAT(FT_ERR_PREFIX, e)");
    }
    if (streq(case_id, "fttypes.FT_ERR.default_prefix_resolves_error_symbol")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        printf("{\"name\":\"Ok\",\"resolved_error\":%d},", FT_ERR(Ok));
        printf("{\"name\":\"Invalid_Argument\",\"resolved_error\":%d},", FT_ERR(Invalid_Argument));
        printf("{\"name\":\"Invalid_Face_Handle\",\"resolved_error\":%d},", FT_ERR(Invalid_Face_Handle));
        printf("{\"name\":\"Cannot_Open_Resource\",\"resolved_error\":%d}", FT_ERR(Cannot_Open_Resource));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR.used_by_error_comparison_macros")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        printf("{\"macro\":\"FT_ERR_EQ\",\"error\":\"Invalid_Argument\",\"result\":");
        print_json_bool(FT_ERR_EQ(FT_ERR(Invalid_Argument), Invalid_Argument));
        printf("},");
        printf("{\"macro\":\"FT_ERR_NEQ\",\"error\":\"Invalid_Argument\",\"result\":");
        print_json_bool(FT_ERR_NEQ(FT_ERR(Invalid_Argument), Invalid_Argument));
        printf("},");
        printf("{\"macro\":\"FT_ERR_EQ\",\"error\":\"Ok\",\"result\":");
        print_json_bool(FT_ERR_EQ(FT_ERR(Ok), Ok));
        printf("},");
        printf("{\"macro\":\"FT_ERR_NEQ\",\"error\":\"Ok\",\"result\":");
        print_json_bool(FT_ERR_NEQ(FT_ERR(Ok), Ok));
        printf("}");
        printf("]}}\n");
        return 0;
    }

    if (streq(case_id, "fttypes.FT_ERR_EQ.macro_import_contract")) {
        return emit_macro_import_contract("FT_ERROR_BASE(x) == FT_ERROR_BASE(FT_ERR(e))");
    }
    if (streq(case_id, "fttypes.FT_ERR_EQ.ignores_module_bits_for_equal_base")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_error_pair_row(0x1200 | FT_Err_Invalid_Argument, "Invalid_Argument", FT_ERR_EQ(0x1200 | FT_Err_Invalid_Argument, Invalid_Argument));
        printf(",");
        print_error_pair_row(0x3400 | FT_Err_Invalid_Face_Handle, "Invalid_Face_Handle", FT_ERR_EQ(0x3400 | FT_Err_Invalid_Face_Handle, Invalid_Face_Handle));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR_EQ.distinguishes_different_base_codes")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_error_pair_row(0x1200 | FT_Err_Invalid_Argument, "Invalid_Face_Handle", FT_ERR_EQ(0x1200 | FT_Err_Invalid_Argument, Invalid_Face_Handle));
        printf(",");
        print_error_pair_row(0x1200 | FT_Err_Invalid_Face_Handle, "Invalid_Argument", FT_ERR_EQ(0x1200 | FT_Err_Invalid_Face_Handle, Invalid_Argument));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR_EQ.ok_error_comparison")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_error_pair_row(0, "Ok", FT_ERR_EQ(0, Ok));
        printf(",");
        print_error_pair_row(FT_Err_Invalid_Argument, "Ok", FT_ERR_EQ(FT_Err_Invalid_Argument, Ok));
        printf(",");
        print_error_pair_row(256, "Ok", FT_ERR_EQ(256, Ok));
        printf("]}}\n");
        return 0;
    }

    if (streq(case_id, "fttypes.FT_ERR_NEQ.macro_import_contract")) {
        return emit_macro_import_contract("FT_ERROR_BASE(x) != FT_ERROR_BASE(FT_ERR(e))");
    }
    if (streq(case_id, "fttypes.FT_ERR_NEQ.ignores_module_bits_for_equal_base")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_error_pair_row(0x1200 | FT_Err_Invalid_Argument, "Invalid_Argument", FT_ERR_NEQ(0x1200 | FT_Err_Invalid_Argument, Invalid_Argument));
        printf(",");
        print_error_pair_row(0x3400 | FT_Err_Invalid_Face_Handle, "Invalid_Face_Handle", FT_ERR_NEQ(0x3400 | FT_Err_Invalid_Face_Handle, Invalid_Face_Handle));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR_NEQ.distinguishes_different_base_codes")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_error_pair_row(0x1200 | FT_Err_Invalid_Argument, "Invalid_Face_Handle", FT_ERR_NEQ(0x1200 | FT_Err_Invalid_Argument, Invalid_Face_Handle));
        printf(",");
        print_error_pair_row(0x1200 | FT_Err_Invalid_Face_Handle, "Invalid_Argument", FT_ERR_NEQ(0x1200 | FT_Err_Invalid_Face_Handle, Invalid_Argument));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR_NEQ.ok_error_comparison")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_error_pair_row(0, "Ok", FT_ERR_NEQ(0, Ok));
        printf(",");
        print_error_pair_row(FT_Err_Invalid_Argument, "Ok", FT_ERR_NEQ(FT_Err_Invalid_Argument, Ok));
        printf(",");
        print_error_pair_row(256, "Ok", FT_ERR_NEQ(256, Ok));
        printf("]}}\n");
        return 0;
    }

    if (streq(case_id, "fttypes.FT_ERR_CAT.macro_import_contract")) {
        return emit_macro_import_contract("FT_ERR_XCAT(x, y)");
    }
    if (streq(case_id, "fttypes.FT_ERR_XCAT.macro_import_contract")) {
        return emit_macro_import_contract("x ## y");
    }

    if (streq(case_id, "fttypes.FT_MAKE_TAG.macro_import_contract")) {
        return emit_macro_import_contract("byte casts shifted 24,16,8,0 into FT_Tag");
    }
    if (streq(case_id, "fttypes.FT_MAKE_TAG.standard_sfnt_tags")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_tag_row("name", FT_MAKE_TAG('n', 'a', 'm', 'e'));
        printf(",");
        print_tag_row("glyf", FT_MAKE_TAG('g', 'l', 'y', 'f'));
        printf(",");
        print_tag_row("head", FT_MAKE_TAG('h', 'e', 'a', 'd'));
        printf(",");
        print_tag_row("cmap", FT_MAKE_TAG('c', 'm', 'a', 'p'));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_MAKE_TAG.byte_order_big_endian")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_tag_row("positional_bits", FT_MAKE_TAG(1, 2, 3, 4));
        printf(",");
        print_tag_row("first_only", FT_MAKE_TAG(1, 0, 0, 0));
        printf(",");
        print_tag_row("last_only", FT_MAKE_TAG(0, 0, 0, 1));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_MAKE_TAG.high_bit_bytes_do_not_sign_extend")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        print_tag_row("all_high", FT_MAKE_TAG(255, 254, 128, 127));
        printf(",");
        print_tag_row("first_high", FT_MAKE_TAG(128, 0, 1, 2));
        printf(",");
        print_tag_row("last_high", FT_MAKE_TAG(0, 1, 2, 255));
        printf("]}}\n");
        return 0;
    }

    if (streq(case_id, "fttypes.FT_IS_EMPTY.macro_import_contract")) {
        return emit_macro_import_contract("((list).head == 0)");
    }
    if (streq(case_id, "fttypes.FT_IS_EMPTY.empty_when_head_null") ||
        streq(case_id, "fttypes.FT_IS_EMPTY.tail_is_not_considered")) {
        FT_ListNodeRec tail_node;
        FT_ListRec list;
        list.head = NULL;
        list.tail = streq(case_id, "fttypes.FT_IS_EMPTY.tail_is_not_considered") ? &tail_node : NULL;
        print_ok_output_prefix();
        printf("{\"head_null\":true,\"tail_null\":");
        print_json_bool(list.tail == NULL);
        printf(",\"result\":");
        print_json_bool(FT_IS_EMPTY(list));
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_IS_EMPTY.non_empty_when_head_nonnull")) {
        FT_ListNodeRec head_node;
        FT_ListNodeRec tail_node;
        FT_ListRec list_a = { &head_node, NULL };
        FT_ListRec list_b = { &head_node, &tail_node };
        print_ok_output_prefix();
        printf("{\"rows\":[");
        printf("{\"head_null\":false,\"tail_null\":true,\"result\":");
        print_json_bool(FT_IS_EMPTY(list_a));
        printf("},");
        printf("{\"head_null\":false,\"tail_null\":false,\"result\":");
        print_json_bool(FT_IS_EMPTY(list_b));
        printf("}");
        printf("]}}\n");
        return 0;
    }

    if (streq(case_id, "ftimage.FT_IMAGE_TAG.expansion_matches_header")) {
        print_ok_output_prefix();
        printf("{\"macro\":\"FT_IMAGE_TAG\",\"value\":%ld,\"import_compiles\":true}}\n", (long)FT_GLYPH_FORMAT_OUTLINE);
        return 0;
    }
    if (streq(case_id, "ftimage.FT_IMAGE_TAG.glyph_format_values_match_c")) {
        print_ok_output_prefix();
        printf("{\"values\":{");
        printf("\"FT_GLYPH_FORMAT_NONE\":%ld,", (long)FT_GLYPH_FORMAT_NONE);
        printf("\"FT_GLYPH_FORMAT_COMPOSITE\":%ld,", (long)FT_GLYPH_FORMAT_COMPOSITE);
        printf("\"FT_GLYPH_FORMAT_BITMAP\":%ld,", (long)FT_GLYPH_FORMAT_BITMAP);
        printf("\"FT_GLYPH_FORMAT_OUTLINE\":%ld,", (long)FT_GLYPH_FORMAT_OUTLINE);
        printf("\"FT_GLYPH_FORMAT_PLOTTER\":%ld,", (long)FT_GLYPH_FORMAT_PLOTTER);
        printf("\"FT_GLYPH_FORMAT_SVG\":%ld", (long)FT_GLYPH_FORMAT_SVG);
        printf("},\"import_compiles\":true}}\n");
        return 0;
    }
    if (streq(case_id, "ftimage.FT_CURVE_TAG.expansion_matches_header")) {
        char values[] = {0, 1, 2, 4, 9, 18};
        print_ok_output_prefix();
        printf("{\"macro\":\"FT_CURVE_TAG\",\"values\":[");
        for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
            if (i) printf(",");
            printf("%d", FT_CURVE_TAG(values[i]));
        }
        printf("],\"import_compiles\":true}}\n");
        return 0;
    }

    fprintf(stderr, "unsupported macro eval: %s\n", case_id);
    return 2;
}

static void print_bitmap(FT_GlyphSlot slot) {
    long len = 0;
    if (slot->bitmap.buffer && slot->bitmap.rows > 0) {
        len = labs(slot->bitmap.pitch) * slot->bitmap.rows;
    }
    printf("\"bitmap\":");
    if (!slot->bitmap.buffer || len == 0) {
        printf("null");
        return;
    }
    printf("{\"width\":%u,\"rows\":%u,\"pitch\":%d,\"pixel_mode\":%u,\"num_grays\":%u,\"left\":%d,\"top\":%d,\"buffer_hex\":\"",
           slot->bitmap.width,
           slot->bitmap.rows,
           slot->bitmap.pitch,
           slot->bitmap.pixel_mode,
           slot->bitmap.num_grays,
           slot->bitmap_left,
           slot->bitmap_top);
    print_hex_bytes(slot->bitmap.buffer, len);
    printf("\"}");
}

static void print_slot(FT_GlyphSlot slot, FT_UInt glyph_index) {
    printf("\"output\":{");
    printf("\"glyph_index\":%u,", glyph_index);
    printf("\"format\":%ld,", (long)slot->format);
    printf("\"advance\":{\"x\":%ld,\"y\":%ld},", slot->advance.x, slot->advance.y);
    printf("\"metrics\":{");
    printf("\"width\":%ld,\"height\":%ld,\"horiBearingX\":%ld,\"horiBearingY\":%ld,\"horiAdvance\":%ld,\"vertBearingX\":%ld,\"vertBearingY\":%ld,\"vertAdvance\":%ld",
           slot->metrics.width,
           slot->metrics.height,
           slot->metrics.horiBearingX,
           slot->metrics.horiBearingY,
           slot->metrics.horiAdvance,
           slot->metrics.vertBearingX,
           slot->metrics.vertBearingY,
           slot->metrics.vertAdvance);
    printf("},");
    print_bitmap(slot);
    printf("}");
}

static void print_size_metrics(FT_Size_Metrics metrics) {
    printf("\"output\":{");
    printf("\"x_ppem\":%u,\"y_ppem\":%u,\"x_scale\":%ld,\"y_scale\":%ld,\"ascender\":%ld,\"descender\":%ld,\"height\":%ld,\"max_advance\":%ld",
           metrics.x_ppem,
           metrics.y_ppem,
           metrics.x_scale,
           metrics.y_scale,
           metrics.ascender,
           metrics.descender,
           metrics.height,
           metrics.max_advance);
    printf("}");
}

static void print_advance(FT_Fixed advance) {
    printf("\"output\":{\"advance\":%ld}", (long)advance);
}

static void print_advances(const FT_Fixed* advances, FT_UInt count) {
    printf("\"output\":{\"advances\":[");
    for (FT_UInt i = 0; i < count; i++) {
        if (i) {
            printf(",");
        }
        printf("%ld", (long)advances[i]);
    }
    printf("]}");
}

static int emit_face_or_slot(int argc, char** argv) {
    const char* command = argv[1];
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[6], NULL, 10);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        free(data);
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    FT_Face face;
    err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    if (!err && face_index < 0 && streq(command, "--new-memory-face")) {
        printf("{");
        print_status(err);
        printf(",\"output\":{\"opened\":true}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err) {
        err = FT_Set_Pixel_Sizes(face, pixel_width, pixel_height);
    }

    printf("{");
    if (err || streq(command, "--new-memory-face") || streq(command, "--set-pixel-sizes") || streq(command, "--size-metrics")) {
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else if (streq(command, "--set-pixel-sizes") || streq(command, "--size-metrics")) {
            printf(",");
            print_size_metrics(face->size->metrics);
            printf("}\n");
        } else {
            printf(",\"output\":{\"opened\":true}}\n");
        }
        if (!err) {
            FT_Done_Face(face);
        }
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-char-index")) {
        FT_ULong char_code = strtoul(argv[7], NULL, 10);
        print_status(0);
        printf(",\"output\":{\"value\":%u}}\n", FT_Get_Char_Index(face, char_code));
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-advance")) {
        FT_UInt glyph_index = (FT_UInt)strtoul(argv[7], NULL, 10);
        FT_Int32 load_flags = (FT_Int32)strtol(argv[8], NULL, 10);
        FT_Fixed advance = 0;
        err = FT_Get_Advance(face, glyph_index, load_flags, &advance);
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",");
            print_advance(advance);
            printf("}\n");
        }
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-advances")) {
        FT_UInt start = (FT_UInt)strtoul(argv[7], NULL, 10);
        FT_UInt count = (FT_UInt)strtoul(argv[8], NULL, 10);
        FT_Int32 load_flags = (FT_Int32)strtol(argv[9], NULL, 10);
        size_t alloc_count = count ? (size_t)count : 1;
        FT_Fixed* advances = (FT_Fixed*)calloc(alloc_count, sizeof(FT_Fixed));
        if (!advances) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        err = FT_Get_Advances(face, start, count, load_flags, advances);
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",");
            print_advances(advances, count);
            printf("}\n");
        }
        free(advances);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    FT_UInt glyph_index = 0;
    FT_Int32 load_flags = 0;
    if (streq(command, "--load-char") || streq(command, "--render-glyph")) {
        FT_ULong char_code = strtoul(argv[7], NULL, 10);
        load_flags = (FT_Int32)strtol(argv[8], NULL, 10);
        glyph_index = FT_Get_Char_Index(face, char_code);
    } else if (streq(command, "--load-glyph") || streq(command, "--render-glyph-index")) {
        glyph_index = (FT_UInt)strtoul(argv[7], NULL, 10);
        load_flags = (FT_Int32)strtol(argv[8], NULL, 10);
    } else {
        fprintf(stderr, "unsupported command: %s\n", command);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 2;
    }

    err = FT_Load_Glyph(face, glyph_index, load_flags);
    if (!err && (streq(command, "--render-glyph") || streq(command, "--render-glyph-index"))) {
        FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[9], NULL, 10);
        err = FT_Render_Glyph(face->glyph, render_mode);
    }
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",");
        print_slot(face->glyph, glyph_index);
        printf("}\n");
    }

    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_set_char_size(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    FT_F26Dot6 char_width = (FT_F26Dot6)strtol(argv[5], NULL, 10);
    FT_F26Dot6 char_height = (FT_F26Dot6)strtol(argv[6], NULL, 10);
    FT_UInt horz_resolution = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt vert_resolution = (FT_UInt)strtoul(argv[8], NULL, 10);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        return 2;
    }

    FT_Library library;
    FT_Error err = FT_Init_FreeType(&library);
    FT_Face face = NULL;
    if (!err) {
        err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    }
    if (!err) {
        err = FT_Set_Char_Size(face, char_width, char_height, horz_resolution, vert_resolution);
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"set\":true}}\n");
    }
    if (face) {
        FT_Done_Face(face);
    }
    if (library) {
        FT_Done_FreeType(library);
    }
    free(data);
    return 0;
}

static int dispatch(int argc, char** argv) {
    if (argc == 3 && streq(argv[1], "--constant")) {
        return emit_constant(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--layout")) {
        return emit_layout(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--type-probe")) {
        return emit_type_probe(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--function-probe")) {
        return emit_function_probe(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--macro-eval")) {
        return emit_macro_eval(argv[2]);
    }
    if (argc == 7 && (streq(argv[1], "--new-memory-face") || streq(argv[1], "--set-pixel-sizes") || streq(argv[1], "--size-metrics"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--set-char-size")) {
        return emit_set_char_size(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-char-index")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && (streq(argv[1], "--load-char") || streq(argv[1], "--load-glyph"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--get-advance")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--get-advances")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && (streq(argv[1], "--render-glyph") || streq(argv[1], "--render-glyph-index"))) {
        return emit_face_or_slot(argc, argv);
    }
    fprintf(stderr, "usage: gen_unified_oracle --constant SYMBOL | --layout RECORD | --type-probe SYMBOL | --function-probe SYMBOL | --macro-eval CASE_ID | --new-memory-face SRC_KIND SRC FACE_INDEX PX PY | --set-pixel-sizes SRC_KIND SRC FACE_INDEX PX PY | --set-char-size SRC_KIND SRC FACE_INDEX WIDTH HEIGHT HR VR | --size-metrics SRC_KIND SRC FACE_INDEX PX PY | --get-char-index SRC_KIND SRC FACE_INDEX PX PY CHAR | --load-char SRC_KIND SRC FACE_INDEX PX PY CHAR FLAGS | --load-glyph SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --get-advance SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --get-advances SRC_KIND SRC FACE_INDEX PX PY START COUNT FLAGS | --render-glyph SRC_KIND SRC FACE_INDEX PX PY CHAR FLAGS MODE | --render-glyph-index SRC_KIND SRC FACE_INDEX PX PY GID FLAGS MODE\n");
    return 2;
}

static int run_batch_argv(void) {
    char line[16384];
    char* argv[16];
    argv[0] = (char*)"gen_unified_oracle";
    while (fgets(line, sizeof(line), stdin)) {
        int argc = 1;
        char* token = strtok(line, "\t\r\n");
        while (token && argc < 16) {
            argv[argc++] = token;
            token = strtok(NULL, "\t\r\n");
        }
        if (token) {
            fprintf(stderr, "too many batch arguments\n");
            return 2;
        }
        if (argc == 1) {
            continue;
        }
        int status = dispatch(argc, argv);
        fflush(stdout);
        if (status != 0) {
            return status;
        }
    }
    return 0;
}

int main(int argc, char** argv) {
    if (argc == 2 && streq(argv[1], "--batch-argv")) {
        return run_batch_argv();
    }
    return dispatch(argc, argv);
}
