/* fontdone benchmark — real FreeType C vs Rust FFI
 *
 * Compares Rust fontdone::ffi (pure Rust impementation) against
 * pinned FreeType 2.14.3 C library from freetype/build/libfreetype.so.
 *
 * Build:
 *   cd bench-c
 *   cc -I ../freetype/include bench_c.c -L ../freetype/build -lfreetype -lm -o bench-ft
 *   LD_LIBRARY_PATH=../freetype/build ./bench-ft
 */

#define _POSIX_C_SOURCE 200809L
#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_ADVANCES_H
#include FT_SFNT_NAMES_H
#include FT_TRUETYPE_TABLES_H
#include FT_TRUETYPE_IDS_H
#include FT_TRUETYPE_TAGS_H
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* Timing */
static uint64_t now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

/* Sample glyph indices (covering simple, composite, CJK range) */
#define GLYPH_A     36
#define GLYPH_B     37
#define GLYPH_C0   192   /* À — composite */
#define GLYPH_HAN  0x4E2D /* 中 — CJK (if available) */
#define GLYPH_EM   0x1F600 /* 😀 — emoji (if available) */

#define WARMUP  10
#define ITERS   30

static const char* FONT_FILE = NULL;

static double bench(const char* name, void (*f)(FT_Face, void*), FT_Face face, void* ctx) {
    /* Warmup */
    for (int i = 0; i < WARMUP; i++) f(face, ctx);
    /* Measure */
    uint64_t start = now_ns();
    for (int i = 0; i < ITERS; i++) f(face, ctx);
    double ns = (double)(now_ns() - start) / (double)ITERS;
    printf("  %-35s  %10.0f ns\n", name, ns);
    return ns;
}

/* ── Operations ──────────────────────────────────────────────────── */

struct GlyphCtx { FT_UInt gid; FT_Int32 flags; };

static void op_load_glyph(FT_Face face, void* vctx) {
    struct GlyphCtx* ctx = (struct GlyphCtx*)vctx;
    FT_Load_Glyph(face, ctx->gid, ctx->flags);
}

static void op_load_char(FT_Face face, void* vctx) {
    struct GlyphCtx* ctx = (struct GlyphCtx*)vctx;
    FT_Load_Char(face, ctx->gid /* actually char_code */, ctx->flags);
}

static void op_get_char_index(FT_Face face, void* vctx) {
    (void)vctx;
    FT_Get_Char_Index(face, 'A');
    FT_Get_Char_Index(face, 0x4E2D); /* 中 */
}

struct KernCtx { FT_UInt left; FT_UInt right; };

static void op_get_kerning(FT_Face face, void* vctx) {
    struct KernCtx* ctx = (struct KernCtx*)vctx;
    FT_Vector v;
    FT_Get_Kerning(face, ctx->left, ctx->right, FT_KERNING_DEFAULT, &v);
}

static void op_get_advance(FT_Face face, void* vctx) {
    FT_UInt gid = *(FT_UInt*)vctx;
    FT_Fixed adv;
    FT_Get_Advance(face, gid, FT_LOAD_DEFAULT, &adv);
}

struct SfntCtx { FT_UInt idx; };

static void op_sfnt_table_info(FT_Face face, void* vctx) {
    struct SfntCtx* ctx = (struct SfntCtx*)vctx;
    FT_ULong tag, len;
    FT_Sfnt_Table_Info(face, ctx->idx, &tag, &len);
}

static void op_load_sfnt_table(FT_Face face, void* vctx) {
    (void)vctx;
    /* Length probe: buffer=NULL, length=0 → returns table size */
    FT_ULong len = 0;
    FT_Load_Sfnt_Table(face, TTAG_head, 0, NULL, &len);
    /* Now copy some bytes */
    if (len > 54) len = 54;
    unsigned char buf[54];
    FT_ULong copy_len = len;
    FT_Load_Sfnt_Table(face, TTAG_head, 0, buf, &copy_len);
}

static void op_size_metrics(FT_Face face, void* vctx) {
    (void)vctx;
    /* Access size->metrics fields to prevent optimization */
    volatile FT_Pos asc = face->size->metrics.ascender;
    volatile FT_Pos desc = face->size->metrics.descender;
    (void)asc; (void)desc;
}

struct GlyphArrCtx { FT_UInt* glyphs; int count; FT_Int32 flags; };

static void op_10_glyph_load(FT_Face face, void* vctx) {
    struct GlyphArrCtx* ctx = (struct GlyphArrCtx*)vctx;
    for (int i = 0; i < ctx->count; i++) {
        FT_Load_Glyph(face, ctx->glyphs[i], ctx->flags);
    }
}

static void op_string_render(FT_Face face, void* vctx) {
    const char* text = (const char*)vctx;
    FT_Int32 flags = FT_LOAD_RENDER | FT_LOAD_TARGET_NORMAL;
    for (const char* p = text; *p; p++) {
        FT_UInt gid = FT_Get_Char_Index(face, (FT_ULong)(unsigned char)*p);
        if (gid > 0) FT_Load_Glyph(face, gid, flags);
    }
}

static void op_string_advance(FT_Face face, void* vctx) {
    const char* text = (const char*)vctx;
    for (const char* p = text; *p; p++) {
        FT_UInt gid = FT_Get_Char_Index(face, (FT_ULong)(unsigned char)*p);
        if (gid > 0) {
            FT_Fixed adv;
            FT_Get_Advance(face, gid, FT_LOAD_DEFAULT, &adv);
        }
    }
}

static void op_init_free_type(FT_Face face, void* vctx) {
    (void)face; (void)vctx;
    FT_Library lib;
    FT_Init_FreeType(&lib);
    FT_Done_FreeType(lib);
}

/* ── Main ─────────────────────────────────────────────────────────── */

static unsigned char* load_file(const char* path, long* out_len) {
    FILE* fp = fopen(path, "rb");
    if (!fp) { perror(path); return NULL; }
    fseek(fp, 0, SEEK_END);
    *out_len = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    unsigned char* data = (unsigned char*)malloc((size_t)*out_len);
    if (fread(data, 1, (size_t)*out_len, fp) != (size_t)*out_len) {
        perror(path); free(data); fclose(fp); return NULL;
    }
    fclose(fp);
    return data;
}

int main(int argc, char** argv) {
    const char* font_path = (argc > 1) ? argv[1] : "tests/fixtures/input/fonts/DejaVuSans.ttf";
    FONT_FILE = font_path;

    long data_len = 0;
    unsigned char* data = load_file(font_path, &data_len);
    if (!data) return 1;

    float sizes[] = {10.0f, 16.0f, 24.0f, 36.0f, 72.0f};
    int n_sizes = sizeof(sizes) / sizeof(sizes[0]);

    printf("\n  FreeType 2.14.3 C — %d iters/op, %d warmup\n", ITERS, WARMUP);
    printf("  %s\n", font_path);
    printf("  %-80s\n", "══════════════════════════════════════════════════════════════════════════════════════════════");

    /* Open face */
    FT_Library library;
    FT_Init_FreeType(&library);

    for (int s = 0; s < n_sizes; s++) {
        float sz = sizes[s];
        FT_Face face;
        FT_New_Memory_Face(library, data, data_len, 0, &face);
        FT_Set_Pixel_Sizes(face, 0, (FT_UInt)sz);

        printf("\n  ── %.0fpt ──\n", sz);

        struct GlyphCtx ctx = { .gid = GLYPH_A, .flags = FT_LOAD_DEFAULT };
        bench("FT_Load_Glyph (default)",           op_load_glyph, face, &ctx);

        ctx.flags = FT_LOAD_NO_HINTING;
        bench("FT_Load_Glyph (no hinting)",        op_load_glyph, face, &ctx);

        ctx.flags = FT_LOAD_FORCE_AUTOHINT;
        bench("FT_Load_Glyph (force autohint)",    op_load_glyph, face, &ctx);

        ctx.gid = GLYPH_C0; ctx.flags = FT_LOAD_DEFAULT;
        bench("FT_Load_Glyph (composite)",         op_load_glyph, face, &ctx);

        ctx.flags = FT_LOAD_RENDER | FT_LOAD_TARGET_NORMAL;
        ctx.gid = GLYPH_A;
        bench("FT_Load+Render (normal AA)",        op_load_glyph, face, &ctx);

        ctx.flags = FT_LOAD_RENDER | FT_LOAD_TARGET_MONO;
        bench("FT_Load+Render (mono)",             op_load_glyph, face, &ctx);

        ctx.gid = 'A'; ctx.flags = FT_LOAD_DEFAULT;
        bench("FT_Load_Char ('A'=65)",             op_load_char, face, &ctx);

        bench("FT_Get_Char_Index",                 op_get_char_index, face, NULL);

        struct KernCtx kctx = { .left = GLYPH_A, .right = GLYPH_B };
        bench("FT_Get_Kerning",                    op_get_kerning, face, &kctx);

        FT_UInt gid = GLYPH_A;
        bench("FT_Get_Advance",                    op_get_advance, face, &gid);

        bench("FT_Size_Metrics",                   op_size_metrics, face, NULL);

        struct SfntCtx sctx = { .idx = 0 };
        bench("FT_Sfnt_Table_Info",                op_sfnt_table_info, face, &sctx);
        bench("FT_Load_Sfnt_Table",                op_load_sfnt_table, face, NULL);

        /* 10-glyph batch */
        FT_UInt batch[10] = {36,37,42,47,50,56,65,66,67,72};
        struct GlyphArrCtx actx = { .glyphs = batch, .count = 10, .flags = FT_LOAD_DEFAULT };
        bench("10-Glyph load",                     op_10_glyph_load, face, &actx);
        actx.flags = FT_LOAD_RENDER | FT_LOAD_TARGET_NORMAL;
        bench("10-Glyph load+render AA",          op_10_glyph_load, face, &actx);

        /* String rendering */
        const char* txt = "The quick brown fox jumps over the lazy dog.";
        bench("String render (44 chars)",           op_string_render, face, (void*)txt);
        bench("String advance (44 chars)",          op_string_advance, face, (void*)txt);

        const char* para = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod.";
        bench("Paragraph render (78 chars)",        op_string_render, face, (void*)para);
        bench("Paragraph advance (78 chars)",       op_string_advance, face, (void*)para);

        FT_Done_Face(face);
    }

    FT_Done_FreeType(library);
    free(data);
    printf("\n  Done.\n\n");
    return 0;
}
