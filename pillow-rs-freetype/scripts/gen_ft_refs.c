#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ft2build.h>
#include FT_FREETYPE_H
#include <openssl/sha.h>  /* fallback: compute SHA in python later */

static FT_Int32 load_flags_for_mode(const char *mode) {
    if (strcmp(mode, "native") == 0 || strcmp(mode, "native-tt-default") == 0) {
        return FT_LOAD_RENDER;
    }
    if (strcmp(mode, "no-hinting") == 0) {
        return FT_LOAD_RENDER | FT_LOAD_NO_HINTING;
    }
    return FT_LOAD_RENDER | FT_LOAD_FORCE_AUTOHINT;
}

/* Simple hex dump of bitmap, then we'll compute SHA in python */
int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr,"Usage: %s <font.ttf> [force-autohint|native|no-hinting]\n",argv[0]); return 1; }
    const char *mode = argc >= 3 ? argv[2] : "force-autohint";
    FT_Int32 load_flags = load_flags_for_mode(mode);
    
    FT_Library library;
    FT_Face face;
    FT_Error err = FT_Init_FreeType(&library);
    if(err) { fprintf(stderr,"Init: %d\n",err); return 1; }
    
    FILE *fp = fopen(argv[1],"rb");
    fseek(fp,0,SEEK_END); long sz = ftell(fp); fseek(fp,0,SEEK_SET);
    unsigned char *buf = malloc(sz);
    fread(buf,1,sz,fp); fclose(fp);
    
    err = FT_New_Memory_Face(library,buf,sz,0,&face);
    if(err) { fprintf(stderr,"Face: %d\n",err); return 1; }
    
    int sizes[] = {10,12,16,20,24};
    for(int si=0;si<5;si++) {
        err = FT_Set_Char_Size(face,sizes[si]<<6,0,72,0);
        if(err) continue;
        printf("SIZE %d\n", sizes[si]);
        printf("METRICS %ld %ld\n", face->size->metrics.ascender>>6, 
               -(face->size->metrics.descender>>6));
        
        for(int c=33;c<127;c++) {
            FT_UInt idx = FT_Get_Char_Index(face,c);
            if(!idx) continue;
            err = FT_Load_Glyph(face,idx,load_flags);
            if(err) continue;
            
            FT_GlyphSlot slot = face->glyph;
            int w = slot->bitmap.width, h = slot->bitmap.rows;
            int left = slot->bitmap_left, top = slot->bitmap_top;
            FT_Long adv = slot->metrics.horiAdvance;
            
            printf("GLYPH %d %d %d %d %d %ld", c, w, h, left, top, adv);
            if(w>0 && h>0 && slot->bitmap.buffer) {
                printf(" PIXELS");
                unsigned char *p = slot->bitmap.buffer;
                for(int i=0;i<w*h;i++) printf(" %02x",p[i]);
            }
            printf("\n");
        }
        printf("END_SIZE\n");
    }
    
    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(buf);
    return 0;
}
