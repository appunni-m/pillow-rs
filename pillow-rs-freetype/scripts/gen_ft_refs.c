#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_BBOX_H
#include FT_GLYPH_H
#include FT_OUTLINE_H

#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct FixtureFamily_ {
    const char* name;
    FT_Int32 load_flags;
    const char* load_flags_json;
    FT_Render_Mode render_mode;
    const char* render_mode_name;
    int render_after_load;
    int cbox_after_load;
} FixtureFamily;

/* The vendored FreeType tree can be patched to call this debug hook from
 * ftgrays.c.  Fixture generation does not need cell dumps, but the executable
 * must satisfy the shared library symbol when that patch is present.
 */
void gray_dump_cells(void* raster) {
    (void)raster;
}

static int streq(const char* a, const char* b) {
    return strcmp(a, b) == 0;
}

static FixtureFamily family_from_name(const char* name) {
    if (streq(name, "native") || streq(name, "native-tt-default") ||
        streq(name, "native_tt_default")) {
        return (FixtureFamily){"native_tt_default",
                               FT_LOAD_RENDER,
                               "[\"FT_LOAD_RENDER\"]",
                               FT_RENDER_MODE_NORMAL,
                               "FT_RENDER_MODE_NORMAL",
                               0,
                               0};
    }
    if (streq(name, "force-autohint") || streq(name, "force_autohint") ||
        streq(name, "autohint")) {
        return (FixtureFamily){"force_autohint",
                               FT_LOAD_RENDER | FT_LOAD_FORCE_AUTOHINT,
                               "[\"FT_LOAD_RENDER\",\"FT_LOAD_FORCE_AUTOHINT\"]",
                               FT_RENDER_MODE_NORMAL,
                               "FT_RENDER_MODE_NORMAL",
                               0,
                               0};
    }
    if (streq(name, "no-hinting") || streq(name, "no_hinting")) {
        return (FixtureFamily){"no_hinting",
                               FT_LOAD_RENDER | FT_LOAD_NO_HINTING,
                               "[\"FT_LOAD_RENDER\",\"FT_LOAD_NO_HINTING\"]",
                               FT_RENDER_MODE_NORMAL,
                               "FT_RENDER_MODE_NORMAL",
                               0,
                               0};
    }
    if (streq(name, "metrics-only") || streq(name, "metrics_only")) {
        return (FixtureFamily){"metrics_only",
                               FT_LOAD_DEFAULT,
                               "[\"FT_LOAD_DEFAULT\"]",
                               FT_RENDER_MODE_NORMAL,
                               "none",
                               0,
                               0};
    }
    if (streq(name, "outline-cbox") || streq(name, "outline_cbox")) {
        return (FixtureFamily){"outline_cbox",
                               FT_LOAD_NO_BITMAP,
                               "[\"FT_LOAD_NO_BITMAP\"]",
                               FT_RENDER_MODE_NORMAL,
                               "none",
                               0,
                               1};
    }
    if (streq(name, "render-mono") || streq(name, "render_mono")) {
        return (FixtureFamily){"render_mono",
                               FT_LOAD_TARGET_MONO,
                               "[\"FT_LOAD_TARGET_MONO\"]",
                               FT_RENDER_MODE_MONO,
                               "FT_RENDER_MODE_MONO",
                               1,
                               0};
    }
    if (streq(name, "render-lcd") || streq(name, "render_lcd")) {
        return (FixtureFamily){"render_lcd",
                               FT_LOAD_TARGET_LCD,
                               "[\"FT_LOAD_TARGET_LCD\"]",
                               FT_RENDER_MODE_LCD,
                               "FT_RENDER_MODE_LCD",
                               1,
                               0};
    }
    if (streq(name, "render-lcd-v") || streq(name, "render_lcd_v")) {
        return (FixtureFamily){"render_lcd_v",
                               FT_LOAD_TARGET_LCD_V,
                               "[\"FT_LOAD_TARGET_LCD_V\"]",
                               FT_RENDER_MODE_LCD_V,
                               "FT_RENDER_MODE_LCD_V",
                               1,
                               0};
    }
    return family_from_name("force_autohint");
}

static const char* basename_const(const char* path) {
    const char* slash = strrchr(path, '/');
    return slash ? slash + 1 : path;
}

static int parse_codepoint(const char* text) {
    char* end = NULL;
    long value = strtol(text, &end, 16);
    if (end && *end == '\0') {
        return (int)value;
    }
    return atoi(text);
}

static void print_hex_bytes(const unsigned char* bytes, long len) {
    for (long i = 0; i < len; i++) {
        printf("%02x", bytes[i]);
    }
}

static void print_json_hex_string(const unsigned char* bytes, long len) {
    printf("\"");
    print_hex_bytes(bytes, len);
    printf("\"");
}

static void print_json_string(const char* value) {
    printf("\"");
    for (const unsigned char* p = (const unsigned char*)value; *p; p++) {
        if (*p == '"' || *p == '\\') {
            printf("\\%c", *p);
        } else if (*p >= 0x20 && *p < 0x7f) {
            putchar(*p);
        } else {
            printf("\\u%04x", *p);
        }
    }
    printf("\"");
}

static void print_bbox_json(const char* name, FT_BBox box) {
    printf("\"%s\":{\"x_min\":%ld,\"y_min\":%ld,\"x_max\":%ld,\"y_max\":%ld}",
           name,
           box.xMin,
           box.yMin,
           box.xMax,
           box.yMax);
}

static int load_font_file(const char* path, unsigned char** out, long* out_len) {
    FILE* fp = fopen(path, "rb");
    if (!fp) {
        fprintf(stderr, "open font failed: %s\n", path);
        return 1;
    }
    if (fseek(fp, 0, SEEK_END) != 0) {
        fclose(fp);
        return 1;
    }
    long sz = ftell(fp);
    if (sz < 0 || fseek(fp, 0, SEEK_SET) != 0) {
        fclose(fp);
        return 1;
    }
    unsigned char* buf = (unsigned char*)malloc((size_t)sz);
    if (!buf) {
        fclose(fp);
        return 1;
    }
    if (fread(buf, 1, (size_t)sz, fp) != (size_t)sz) {
        free(buf);
        fclose(fp);
        return 1;
    }
    fclose(fp);
    *out = buf;
    *out_len = sz;
    return 0;
}

static void print_legacy_line(FT_GlyphSlot slot) {
    int w = slot->bitmap.width;
    int h = slot->bitmap.rows;
    int left = slot->bitmap_left;
    int top = slot->bitmap_top;
    printf("%d %d %d %d ", w, h, left, top);
    if (slot->bitmap.buffer && slot->bitmap.pitch >= 0) {
        for (int y = 0; y < h; y++) {
            print_hex_bytes(slot->bitmap.buffer + y * slot->bitmap.pitch, w);
        }
    }
    printf("\n");
}

static int emit_one(const char* font_path,
                    int codepoint,
                    int size,
                    FixtureFamily family,
                    int json_mode) {
    FT_Library library;
    FT_Face face;
    unsigned char* buf = NULL;
    long sz = 0;
    int ft_major = 0, ft_minor = 0, ft_patch = 0;

    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        fprintf(stderr, "FT_Init_FreeType: %d\n", err);
        return 1;
    }
    FT_Library_Version(library, &ft_major, &ft_minor, &ft_patch);

    if (load_font_file(font_path, &buf, &sz) != 0) {
        FT_Done_FreeType(library);
        return 1;
    }

    err = FT_New_Memory_Face(library, buf, sz, 0, &face);
    if (err) {
        fprintf(stderr, "FT_New_Memory_Face: %d\n", err);
        free(buf);
        FT_Done_FreeType(library);
        return 1;
    }

    err = FT_Set_Char_Size(face, size << 6, 0, 72, 0);
    if (err) {
        fprintf(stderr, "FT_Set_Char_Size: %d\n", err);
        FT_Done_Face(face);
        free(buf);
        FT_Done_FreeType(library);
        return 1;
    }

    FT_UInt glyph_index = FT_Get_Char_Index(face, (FT_ULong)codepoint);
    if (!glyph_index) {
        if (json_mode) {
            printf("{\"status\":\"missing_glyph\",\"codepoint\":%d}\n", codepoint);
        } else {
            printf("NO_GLYPH\n");
        }
        FT_Done_Face(face);
        free(buf);
        FT_Done_FreeType(library);
        return 0;
    }

    err = FT_Load_Glyph(face, glyph_index, family.load_flags);
    if (!err && family.render_after_load) {
        err = FT_Render_Glyph(face->glyph, family.render_mode);
    }
    if (err) {
        fprintf(stderr, "FT_Load_Glyph/FT_Render_Glyph: %d\n", err);
        FT_Done_Face(face);
        free(buf);
        FT_Done_FreeType(library);
        return 1;
    }

    FT_GlyphSlot slot = face->glyph;
    if (!json_mode) {
        print_legacy_line(slot);
        FT_Done_Face(face);
        free(buf);
        FT_Done_FreeType(library);
        return 0;
    }

    FT_BBox outline_cbox = {0, 0, 0, 0};
    FT_BBox outline_bbox = {0, 0, 0, 0};
    if (slot->format == FT_GLYPH_FORMAT_OUTLINE) {
        FT_Outline_Get_CBox(&slot->outline, &outline_cbox);
        FT_Outline_Get_BBox(&slot->outline, &outline_bbox);
    }

    long raw_len = 0;
    if (slot->bitmap.buffer && slot->bitmap.rows > 0) {
        raw_len = labs(slot->bitmap.pitch) * slot->bitmap.rows;
    }

    printf("{");
    printf("\"generator\":\"scripts/gen_ft_refs.c\",");
    printf("\"freetype_version\":\"%d.%d.%d\",", ft_major, ft_minor, ft_patch);
    printf("\"fixture_family\":");
    print_json_string(family.name);
    printf(",");
    printf("\"load_flags\":%s,", family.load_flags_json);
    printf("\"load_flags_value\":%ld,", (long)family.load_flags);
    printf("\"render_mode\":");
    print_json_string(family.render_mode_name);
    printf(",");
    printf("\"font\":");
    print_json_string(basename_const(font_path));
    printf(",");
    printf("\"font_path\":");
    print_json_string(font_path);
    printf(",");
    printf("\"size_pt\":%d,", size);
    printf("\"codepoint\":%d,", codepoint);
    printf("\"glyph_index\":%u,", glyph_index);
    printf("\"metrics\":{");
    printf("\"width\":%ld,", slot->metrics.width);
    printf("\"height\":%ld,", slot->metrics.height);
    printf("\"hori_bearing_x\":%ld,", slot->metrics.horiBearingX);
    printf("\"hori_bearing_y\":%ld,", slot->metrics.horiBearingY);
    printf("\"hori_advance\":%ld,", slot->metrics.horiAdvance);
    printf("\"vert_bearing_x\":%ld,", slot->metrics.vertBearingX);
    printf("\"vert_bearing_y\":%ld,", slot->metrics.vertBearingY);
    printf("\"vert_advance\":%ld", slot->metrics.vertAdvance);
    printf("},");
    printf("\"bbox\":{");
    print_bbox_json("outline_cbox_26_6", outline_cbox);
    printf(",");
    print_bbox_json("outline_bbox_26_6", outline_bbox);
    printf(",");
    printf("\"bitmap_pixels\":{\"x_min\":%d,\"y_min\":%d,\"x_max\":%d,\"y_max\":%d}",
           slot->bitmap_left,
           slot->bitmap_top - (int)slot->bitmap.rows,
           slot->bitmap_left + (int)slot->bitmap.width,
           slot->bitmap_top);
    printf("},");
    printf("\"bitmap\":{");
    printf("\"width\":%u,", slot->bitmap.width);
    printf("\"rows\":%u,", slot->bitmap.rows);
    printf("\"pitch\":%d,", slot->bitmap.pitch);
    printf("\"pixel_mode\":%u,", slot->bitmap.pixel_mode);
    printf("\"num_grays\":%u,", slot->bitmap.num_grays);
    printf("\"left\":%d,", slot->bitmap_left);
    printf("\"top\":%d", slot->bitmap_top);
    printf("},");
    printf("\"raw_pixels\":");
    if (slot->bitmap.buffer && raw_len > 0) {
        print_json_hex_string(slot->bitmap.buffer, raw_len);
    } else {
        printf("\"\"");
    }
    printf("}\n");

    FT_Done_Face(face);
    free(buf);
    FT_Done_FreeType(library);
    return 0;
}

int main(int argc, char** argv) {
    int json_mode = 0;
    int arg = 1;
    if (argc > 1 && streq(argv[1], "--json")) {
        json_mode = 1;
        arg = 2;
    }
    if (argc - arg < 3) {
        fprintf(stderr,
                "Usage: %s [--json] <font.ttf> <CP_HEX> <size_pt> [fixture_family]\n",
                argv[0]);
        return 1;
    }

    const char* font_path = argv[arg];
    int codepoint = parse_codepoint(argv[arg + 1]);
    int size = atoi(argv[arg + 2]);
    const char* family_name = (argc - arg >= 4) ? argv[arg + 3] : "force_autohint";
    FixtureFamily family = family_from_name(family_name);

    return emit_one(font_path, codepoint, size, family, json_mode);
}
