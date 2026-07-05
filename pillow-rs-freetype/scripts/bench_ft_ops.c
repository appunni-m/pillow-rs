#define _POSIX_C_SOURCE 200809L

#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_GLYPH_H

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* Batched benchmark helper for scripts/bench_freetype.py.
 *
 * This program is oracle/benchmark tooling only. It is not linked into the
 * Rust runtime crate. To keep the helper dependency-light, it supports the
 * seed matrix shape used by perf_operation_matrix.json and emits JSONL rows.
 */

typedef struct Row_ {
    char id[128];
    char operation[32];
    char font[256];
    char text[16];
    char load_mode[32];
    char render_mode[32];
    double size;
    long iterations;
} Row;

static uint64_t now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ull + (uint64_t)ts.tv_nsec;
}

static uint64_t fnv1a(const unsigned char* data, size_t len) {
    uint64_t hash = 1469598103934665603ull;
    for (size_t i = 0; i < len; i++) {
        hash ^= data[i];
        hash *= 1099511628211ull;
    }
    return hash;
}

static void print_fake_sha(uint64_t hash) {
    /* The Python merger compares this only for C-supported rows once the C
     * helper grows SHA-256 parity. For now it remains deterministic metadata.
     */
    printf("%016llx", (unsigned long long)hash);
}

static int contains(const char* haystack, const char* needle) {
    return strstr(haystack, needle) != NULL;
}

static int extract_string(const char* object, const char* key, char* out, size_t out_len) {
    char pattern[64];
    snprintf(pattern, sizeof(pattern), "\"%s\"", key);
    const char* p = strstr(object, pattern);
    if (!p) {
        if (out_len) {
            out[0] = '\0';
        }
        return 0;
    }
    p = strchr(p + strlen(pattern), ':');
    if (!p) {
        return 0;
    }
    p++;
    while (*p == ' ' || *p == '\t') {
        p++;
    }
    if (*p != '"') {
        return 0;
    }
    p++;
    size_t n = 0;
    while (*p && *p != '"' && n + 1 < out_len) {
        if (*p == '\\' && p[1] == 'u') {
            char* end = NULL;
            long cp = strtol(p + 2, &end, 16);
            if (end == p + 6 && cp > 0) {
                if (cp < 0x80 && n + 1 < out_len) {
                    out[n++] = (char)cp;
                } else if (cp < 0x800 && n + 2 < out_len) {
                    out[n++] = (char)(0xC0 | (cp >> 6));
                    out[n++] = (char)(0x80 | (cp & 0x3F));
                } else if (n + 3 < out_len) {
                    out[n++] = (char)(0xE0 | (cp >> 12));
                    out[n++] = (char)(0x80 | ((cp >> 6) & 0x3F));
                    out[n++] = (char)(0x80 | (cp & 0x3F));
                }
                p = end;
                continue;
            }
        }
        if (*p == '\\' && p[1]) {
            p++;
        }
        out[n++] = *p++;
    }
    out[n] = '\0';
    return 1;
}

static int extract_long(const char* object, const char* key, long* out) {
    char pattern[64];
    snprintf(pattern, sizeof(pattern), "\"%s\"", key);
    const char* p = strstr(object, pattern);
    if (!p) {
        return 0;
    }
    p = strchr(p + strlen(pattern), ':');
    if (!p) {
        return 0;
    }
    *out = strtol(p + 1, NULL, 10);
    return 1;
}

static int extract_double(const char* object, const char* key, double* out) {
    char pattern[64];
    snprintf(pattern, sizeof(pattern), "\"%s\"", key);
    const char* p = strstr(object, pattern);
    if (!p) {
        return 0;
    }
    p = strchr(p + strlen(pattern), ':');
    if (!p) {
        return 0;
    }
    *out = strtod(p + 1, NULL);
    return 1;
}

static int next_codepoint(const char** cursor) {
    const unsigned char* s = (const unsigned char*)*cursor;
    if (!s[0]) {
        return 0;
    }
    if (s[0] < 0x80) {
        *cursor += 1;
        return s[0];
    }
    if ((s[0] & 0xE0) == 0xC0) {
        *cursor += 2;
        return ((s[0] & 0x1F) << 6) | (s[1] & 0x3F);
    }
    if ((s[0] & 0xF0) == 0xE0) {
        *cursor += 3;
        return ((s[0] & 0x0F) << 12) | ((s[1] & 0x3F) << 6) | (s[2] & 0x3F);
    }
    *cursor += 1;
    return '?';
}

static FT_Int32 load_flags_for_row(const Row* row) {
    FT_Int32 flags = FT_LOAD_RENDER;
    if (contains(row->load_mode, "force")) {
        flags |= FT_LOAD_FORCE_AUTOHINT;
    }
    if (strcmp(row->render_mode, "mono") == 0) {
        flags |= FT_LOAD_TARGET_MONO;
    } else if (strcmp(row->render_mode, "lcd") == 0) {
        flags |= FT_LOAD_TARGET_LCD;
    }
    return flags;
}

static void run_row(const char* root, const Row* row) {
    char path[512];
    snprintf(path, sizeof(path), "%s/%s", root, row->font);

    FT_Library library;
    FT_Face face = NULL;
    FT_Init_FreeType(&library);

    int is_load_font = strcmp(row->operation, "load_font") == 0;
    if (!is_load_font) {
        FT_New_Face(library, path, 0, &face);
        FT_Set_Char_Size(face, 0, (FT_F26Dot6)(row->size * 64.0), 72, 72);
    }

    uint64_t start = now_ns();
    uint64_t hash = 0;
    size_t out_len = 0;

    for (long i = 0; i < row->iterations; i++) {
        if (is_load_font) {
            FT_Face loaded = NULL;
            FT_New_Face(library, path, 0, &loaded);
            FT_Set_Char_Size(loaded, 0, (FT_F26Dot6)(row->size * 64.0), 72, 72);
            hash = (uint64_t)loaded->num_glyphs;
            out_len = sizeof(loaded->num_glyphs);
            FT_Done_Face(loaded);
            continue;
        }

        const char* text = row->text[0] ? row->text : "A";
        const char* cursor = text;
        int cp = next_codepoint(&cursor);
        FT_UInt glyph = FT_Get_Char_Index(face, (FT_ULong)cp);
        FT_Int32 flags = load_flags_for_row(row);
        if (strcmp(row->operation, "getname") == 0) {
            hash = fnv1a((const unsigned char*)face->family_name, strlen(face->family_name));
            hash ^= fnv1a((const unsigned char*)face->style_name, strlen(face->style_name));
            out_len = strlen(face->family_name) + strlen(face->style_name);
        } else if (strcmp(row->operation, "getmetrics") == 0) {
            long values[2] = {face->size->metrics.ascender >> 6,
                              -face->size->metrics.descender >> 6};
            hash = fnv1a((const unsigned char*)values, sizeof(values));
            out_len = sizeof(values);
        } else if (strcmp(row->operation, "getlength") == 0) {
            long advance = 0;
            FT_UInt previous = 0;
            cursor = text;
            while ((cp = next_codepoint(&cursor)) != 0) {
                glyph = FT_Get_Char_Index(face, (FT_ULong)cp);
                if (previous && FT_HAS_KERNING(face)) {
                    FT_Vector kern;
                    FT_Get_Kerning(face, previous, glyph, FT_KERNING_DEFAULT, &kern);
                    advance += kern.x;
                }
                FT_Load_Glyph(face, glyph, FT_LOAD_DEFAULT);
                advance += face->glyph->advance.x;
                previous = glyph;
            }
            hash = fnv1a((const unsigned char*)&advance, sizeof(advance));
            out_len = sizeof(advance);
        } else if (strcmp(row->operation, "getbbox") == 0) {
            long pen_x = 0;
            long min_x = 0;
            long min_y = 0;
            long max_x = 0;
            long max_y = 0;
            int have_box = 0;
            cursor = text;
            while ((cp = next_codepoint(&cursor)) != 0) {
                glyph = FT_Get_Char_Index(face, (FT_ULong)cp);
                FT_Load_Glyph(face, glyph, flags);
                long left = (pen_x >> 6) + face->glyph->bitmap_left;
                long top = face->glyph->bitmap_top;
                long right = left + (long)face->glyph->bitmap.width;
                long bottom = top - (long)face->glyph->bitmap.rows;
                if (!have_box || left < min_x) {
                    min_x = left;
                }
                if (!have_box || bottom < min_y) {
                    min_y = bottom;
                }
                if (!have_box || right > max_x) {
                    max_x = right;
                }
                if (!have_box || top > max_y) {
                    max_y = top;
                }
                have_box = 1;
                pen_x += face->glyph->advance.x;
            }
            long values[4] = {min_x, min_y, max_x, max_y};
            hash = fnv1a((const unsigned char*)values, sizeof(values));
            out_len = sizeof(values);
        } else {
            FT_Load_Glyph(face, glyph, flags);
            hash = fnv1a(face->glyph->bitmap.buffer,
                         (size_t)(face->glyph->bitmap.rows * labs(face->glyph->bitmap.pitch)));
            out_len = (size_t)(face->glyph->bitmap.rows * labs(face->glyph->bitmap.pitch));
        }
    }

    uint64_t elapsed = now_ns() - start;
    printf("{\"id\":\"%s\",\"operation\":\"%s\",\"iterations\":%ld,"
           "\"c_ns_total\":%llu,\"c_ns_per_iter\":%llu,"
           "\"output_len\":%zu,\"output_fingerprint\":\"",
           row->id,
           row->operation,
           row->iterations,
           (unsigned long long)elapsed,
           (unsigned long long)(elapsed / (uint64_t)(row->iterations ? row->iterations : 1)),
           out_len);
    print_fake_sha(hash);
    printf("\"}\n");

    if (face) {
        FT_Done_Face(face);
    }
    FT_Done_FreeType(library);
}

int main(int argc, char** argv) {
    if (argc < 2) {
        fprintf(stderr, "usage: %s perf_operation_matrix.json\n", argv[0]);
        return 2;
    }
    FILE* fp = fopen(argv[1], "rb");
    if (!fp) {
        perror(argv[1]);
        return 1;
    }
    fseek(fp, 0, SEEK_END);
    long len = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    char* data = (char*)calloc((size_t)len + 1, 1);
    if (fread(data, 1, (size_t)len, fp) != (size_t)len) {
        perror(argv[1]);
        free(data);
        fclose(fp);
        return 1;
    }
    fclose(fp);

    char root[512];
    strncpy(root, argv[1], sizeof(root) - 1);
    char* marker = strstr(root, "/tests/fixtures/");
    if (marker) {
        *marker = '\0';
    } else {
        strcpy(root, ".");
    }

    const char* p = data;
    while ((p = strchr(p, '{')) != NULL) {
        const char* end = strchr(p, '}');
        if (!end) {
            break;
        }
        size_t object_len = (size_t)(end - p + 1);
        char object[2048];
        if (object_len >= sizeof(object)) {
            p = end + 1;
            continue;
        }
        memcpy(object, p, object_len);
        object[object_len] = '\0';
        if (!contains(object, "\"operation\"")) {
            p = end + 1;
            continue;
        }
        Row row;
        memset(&row, 0, sizeof(row));
        extract_string(object, "id", row.id, sizeof(row.id));
        extract_string(object, "operation", row.operation, sizeof(row.operation));
        extract_string(object, "font", row.font, sizeof(row.font));
        extract_string(object, "text", row.text, sizeof(row.text));
        extract_string(object, "load_mode", row.load_mode, sizeof(row.load_mode));
        extract_string(object, "render_mode", row.render_mode, sizeof(row.render_mode));
        extract_double(object, "size", &row.size);
        extract_long(object, "iterations", &row.iterations);
        if (row.id[0] && row.iterations > 0) {
            run_row(root, &row);
        }
        p = end + 1;
    }

    free(data);
    return 0;
}
