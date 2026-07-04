#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include <ft2build.h>
#include FT_FREETYPE_H

typedef struct PrsFtGlyph {
    int32_t width;
    int32_t height;
    int32_t left;
    int32_t top;
    int32_t advance_26_6;
    uint8_t* pixels;
    uintptr_t pixels_len;
    int32_t error;
} PrsFtGlyph;

int32_t prs_ft_render_native(
    const uint8_t* data,
    uintptr_t len,
    int32_t size_px,
    uint32_t codepoint,
    PrsFtGlyph* out
) {
    memset(out, 0, sizeof(*out));

    FT_Library library = NULL;
    FT_Face face = NULL;
    FT_Error error = FT_Init_FreeType(&library);
    if (error) {
        out->error = error;
        return error;
    }

    error = FT_New_Memory_Face(library, data, (FT_Long)len, 0, &face);
    if (!error) {
        error = FT_Set_Char_Size(face, size_px << 6, 0, 72, 0);
    }
    if (!error) {
        FT_UInt glyph_index = FT_Get_Char_Index(face, codepoint);
        error = FT_Load_Glyph(face, glyph_index, FT_LOAD_RENDER);
    }

    if (!error) {
        FT_GlyphSlot slot = face->glyph;
        int width = (int)slot->bitmap.width;
        int rows = (int)slot->bitmap.rows;
        int pitch = slot->bitmap.pitch;
        uintptr_t pixels_len = (uintptr_t)(width * rows);
        uint8_t* pixels = NULL;

        if (pixels_len != 0) {
            pixels = (uint8_t*)malloc(pixels_len);
            if (!pixels) {
                error = 64;
            } else {
                for (int y = 0; y < rows; y++) {
                    const uint8_t* src = slot->bitmap.buffer + y * pitch;
                    memcpy(pixels + (uintptr_t)y * (uintptr_t)width, src, (uintptr_t)width);
                }
            }
        }

        if (!error) {
            out->width = width;
            out->height = rows;
            out->left = slot->bitmap_left;
            out->top = slot->bitmap_top;
            out->advance_26_6 = (int32_t)slot->metrics.horiAdvance;
            out->pixels = pixels;
            out->pixels_len = pixels_len;
        } else {
            free(pixels);
        }
    }

    if (face) {
        FT_Done_Face(face);
    }
    if (library) {
        FT_Done_FreeType(library);
    }

    out->error = error;
    return error;
}

void prs_ft_free(void* ptr) {
    free(ptr);
}
