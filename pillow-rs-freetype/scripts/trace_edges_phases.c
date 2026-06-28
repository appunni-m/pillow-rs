#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ft2build.h>
#include FT_FREETYPE_H

/* Build:
   gcc -o /tmp/trace_edges_phases trace_edges_phases.c \
     -I$HOME/.local/include/freetype2 -I$HOME/.local/include \
     -L$HOME/.local/lib -lfreetype -Wl,-rpath,$HOME/.local/lib
*/

int main(int argc, char **argv) {
    if (argc < 4) {
        fprintf(stderr, "Usage: %s <font.ttf> <size_pt> <char>\n", argv[0]);
        return 1;
    }
    const char *font_path = argv[1];
    int size_pt = atoi(argv[2]);
    char ch = argv[3][0];

    FT_Library library;
    if (FT_Init_FreeType(&library)) {
        fprintf(stderr, "FT_Init_FreeType failed\n");
        return 1;
    }

    /* Load font into memory */
    FILE *fp = fopen(font_path, "rb");
    if (!fp) { fprintf(stderr, "Cannot open %s\n", font_path); return 1; }
    fseek(fp, 0, SEEK_END);
    long fsize = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    unsigned char *buf = malloc(fsize);
    fread(buf, 1, fsize, fp);
    fclose(fp);

    FT_Face face;
    if (FT_New_Memory_Face(library, buf, fsize, 0, &face)) {
        fprintf(stderr, "FT_New_Memory_Face failed\n");
        return 1;
    }

    /* Set size in points at 72dpi */
    FT_Set_Char_Size(face, size_pt << 6, 0, 72, 0);

    /* Look up glyph index */
    FT_UInt glyph_idx = FT_Get_Char_Index(face, (FT_ULong)ch);
    if (glyph_idx == 0) {
        fprintf(stderr, "Glyph '%c' not found\n", ch);
        return 1;
    }

    fprintf(stderr, "[C DRIVER] Loading glyph '%c' (index %u) at %dpt with FORCE_AUTOHINT\n",
            ch, glyph_idx, size_pt);

    /* Load with autohinting — this triggers af_latin_hint_edges */
    FT_Error err = FT_Load_Glyph(face, glyph_idx,
                                  FT_LOAD_RENDER | FT_LOAD_FORCE_AUTOHINT);
    if (err) {
        fprintf(stderr, "FT_Load_Glyph failed: %d\n", err);
    }

    /* Also print the final bitmap info for reference */
    FT_GlyphSlot slot = face->glyph;
    fprintf(stderr, "[C DRIVER] Bitmap: %dx%d left=%d top=%d advance=%ld\n",
            slot->bitmap.width, slot->bitmap.rows,
            slot->bitmap_left, slot->bitmap_top,
            (long)slot->metrics.horiAdvance);

    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(buf);
    return 0;
}
