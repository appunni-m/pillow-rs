#include <ft2build.h>
#include FT_FREETYPE_H

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int streq(const char* a, const char* b) {
    return strcmp(a, b) == 0;
}

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
    long value = 0;
    if (streq(symbol, "FT_LOAD_RENDER")) {
        value = FT_LOAD_RENDER;
    } else if (streq(symbol, "FT_RENDER_MODE_MONO")) {
        value = FT_RENDER_MODE_MONO;
    } else if (streq(symbol, "FT_PIXEL_MODE_GRAY")) {
        value = FT_PIXEL_MODE_GRAY;
    } else {
        fprintf(stderr, "unsupported constant: %s\n", symbol);
        return 2;
    }
    printf("{");
    print_status(0);
    printf(",\"output\":{\"value\":%ld}}\n", value);
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
    } else {
        fprintf(stderr, "unsupported record: %s\n", record);
        return 2;
    }
    printf("]}}\n");
    return 0;
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

    FT_Library library;
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
    if (!err) {
        err = FT_Set_Pixel_Sizes(face, pixel_width, pixel_height);
    }

    printf("{");
    if (err || streq(command, "--new-memory-face")) {
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",\"output\":{\"num_faces\":%ld,\"face_index\":%ld,\"num_glyphs\":%ld}}\n",
                   face->num_faces,
                   face->face_index,
                   face->num_glyphs);
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

    FT_UInt glyph_index = 0;
    FT_Int32 load_flags = 0;
    if (streq(command, "--load-char") || streq(command, "--render-glyph")) {
        FT_ULong char_code = strtoul(argv[7], NULL, 10);
        load_flags = (FT_Int32)strtol(argv[8], NULL, 10);
        glyph_index = FT_Get_Char_Index(face, char_code);
    } else if (streq(command, "--load-glyph")) {
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
    if (!err && streq(command, "--render-glyph")) {
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

int main(int argc, char** argv) {
    if (argc == 3 && streq(argv[1], "--constant")) {
        return emit_constant(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--layout")) {
        return emit_layout(argv[2]);
    }
    if (argc == 7 && streq(argv[1], "--new-memory-face")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-char-index")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && (streq(argv[1], "--load-char") || streq(argv[1], "--load-glyph"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--render-glyph")) {
        return emit_face_or_slot(argc, argv);
    }
    fprintf(stderr, "usage: gen_unified_oracle --constant SYMBOL | --layout RECORD | --new-memory-face SRC_KIND SRC FACE_INDEX PX PY | --get-char-index SRC_KIND SRC FACE_INDEX PX PY CHAR | --load-char SRC_KIND SRC FACE_INDEX PX PY CHAR FLAGS | --load-glyph SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --render-glyph SRC_KIND SRC FACE_INDEX PX PY CHAR FLAGS MODE\n");
    return 2;
}
