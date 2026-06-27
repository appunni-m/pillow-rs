#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ft2build.h>
#include FT_FREETYPE_H
/* We need internal autohinter headers for AF_AxisHints, AF_Edge, etc.
   These are in src/autofit/ and not installed. Include directly. */
#define FT_INTERNAL_OBJECTS_H  /* suppress freetype internal guard */
#include "aftypes.h"
#include "afglobal.h"

/* Build: gcc -o /tmp/trace_edges /tmp/trace_edges.c \
     -I../pillow-rs-freetype/freetype/include \
     -I../pillow-rs-freetype/freetype/src/autofit \
     -L$HOME/.local/lib -lfreetype -Wl,-rpath,$HOME/.local/lib */

int main(int argc, char **argv) {
    if (argc < 4) { fprintf(stderr,"Usage: %s <font.ttf> <size_pt> <char>\n",argv[0]); return 1; }
    const char *path = argv[1];
    int size = atoi(argv[2]);
    char ch = argv[3][0];
    
    FT_Library lib; FT_Face face;
    FT_Init_FreeType(&lib);
    FILE *f = fopen(path,"rb"); fseek(f,0,SEEK_END); long sz = ftell(f); fseek(f,0,SEEK_SET);
    unsigned char *buf = malloc(sz); fread(buf,1,sz,f); fclose(f);
    FT_New_Memory_Face(lib,buf,sz,0,&face);
    FT_Set_Char_Size(face,size<<6,0,72,0);
    
    FT_UInt idx = FT_Get_Char_Index(face,ch);
    FT_Load_Glyph(face,idx,FT_LOAD_RENDER);
    FT_GlyphSlot s = face->glyph;
    
    printf("CHAR: %c (U+%04X)\n", ch, ch);
    printf("GLYPH_INDEX: %u\n", idx);
    printf("SIZE: %dpt\n", size);
    printf("CONTOURS: %d\n", face->glyph->outline.n_contours);
    printf("POINTS: %d\n", face->glyph->outline.n_points);
    printf("BITMAP: %dx%d left=%d top=%d\n", s->bitmap.width, s->bitmap.rows,
           s->bitmap_left, s->bitmap_top);
    printf("ADVANCE_X: %ld (%.2f px)\n", s->metrics.horiAdvance, s->metrics.horiAdvance/64.0);
    
    /* Dump all bitmap pixels */
    if(s->bitmap.width > 0 && s->bitmap.rows > 0) {
        printf("PIXELS:");
        for(int i=0; i<s->bitmap.width*s->bitmap.rows; i++)
            printf(" %02x", s->bitmap.buffer[i]);
        printf("\n");
    }
    
    /* Dump outline points in 26.6 format */
    printf("OUTLINE_POINTS_26_6:");
    for(int i=0; i<face->glyph->outline.n_points; i++) {
        printf(" %ld,%ld,%d", face->glyph->outline.points[i].x,
               face->glyph->outline.points[i].y,
               face->glyph->outline.tags[i] & 1);
    }
    printf("\n");
    
    /* Dump contour ends */
    printf("CONTOUR_ENDS:");
    for(int i=0; i<face->glyph->outline.n_contours; i++)
        printf(" %d", face->glyph->outline.contours[i]);
    printf("\n");
    
    FT_Done_Face(face); FT_Done_FreeType(lib); free(buf);
    return 0;
}
