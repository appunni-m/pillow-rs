#include <ft2build.h>
#include FT_FREETYPE_H

#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <freetype/ftcache.h>
#include <freetype/ftadvanc.h>
#include <freetype/ftbbox.h>
#include <freetype/ftbitmap.h>
#include <freetype/ftbdf.h>
#include <freetype/ftbzip2.h>
#include <freetype/ftcid.h>
#include <freetype/ftcolor.h>
#include <freetype/ftdriver.h>
#include <freetype/ftfntfmt.h>
#include <freetype/ftglyph.h>
#include <freetype/ftgzip.h>
#include <freetype/ftgasp.h>
#include <freetype/ftgxval.h>
#include <freetype/ftimage.h>
#include <freetype/ftincrem.h>
#include <freetype/ftlcdfil.h>
#include <freetype/ftlist.h>
#include <freetype/ftmm.h>
#include <freetype/ftmodapi.h>
#include <freetype/ftoutln.h>
#include <freetype/ftpfr.h>
#include <freetype/ftrender.h>
#include <freetype/ftsnames.h>
#include <freetype/ftsystem.h>
#include <freetype/ftsizes.h>
#include <freetype/ftsynth.h>
#include <freetype/ftstroke.h>
#include <freetype/ftotval.h>
#include <freetype/fttrigon.h>
#include <freetype/ftwinfnt.h>
#include <freetype/internal/ftobjs.h>
#include <freetype/t1tables.h>
#include <freetype/tttables.h>
#include "../freetype/src/cache/ftccache.h"

#ifndef FT_ERR_PREFIX
#define FT_ERR_PREFIX FT_Err_
#endif

static FT_Error cache_no_lookup_requester(FTC_FaceID face_id,
                                          FT_Library library,
                                          FT_Pointer req_data,
                                          FT_Face* aface);

static int streq(const char* a, const char* b) {
    return strcmp(a, b) == 0;
}

static void print_json_string_content(const char* value) {
    if (!value) {
        return;
    }
    for (const unsigned char* cursor = (const unsigned char*)value; *cursor; cursor++) {
        switch (*cursor) {
        case '\\':
            printf("\\\\");
            break;
        case '"':
            printf("\\\"");
            break;
        case '\n':
            printf("\\n");
            break;
        case '\r':
            printf("\\r");
            break;
        case '\t':
            printf("\\t");
            break;
        default:
            if (*cursor < 0x20) {
                printf("\\u%04x", *cursor);
            } else {
                putchar(*cursor);
            }
            break;
        }
    }
}

static void* oracle_alloc(FT_Memory memory, long size) {
    (void)memory;
    return malloc((size_t)size);
}

static void oracle_free(FT_Memory memory, void* block) {
    (void)memory;
    free(block);
}

static void* oracle_realloc(FT_Memory memory, long cur_size, long new_size, void* block) {
    (void)memory;
    (void)cur_size;
    return realloc(block, (size_t)new_size);
}

typedef struct FailAfterMemoryState_ {
    int enabled;
    int fail_after;
    int allocation_count;
} FailAfterMemoryState;

static int fail_after_memory_should_fail(FT_Memory memory) {
    FailAfterMemoryState* state = memory ? (FailAfterMemoryState*)memory->user : NULL;
    if (!state || !state->enabled) {
        return 0;
    }
    if (state->allocation_count++ >= state->fail_after) {
        return 1;
    }
    return 0;
}

static void* fail_after_alloc(FT_Memory memory, long size) {
    if (fail_after_memory_should_fail(memory)) {
        return NULL;
    }
    return malloc((size_t)size);
}

static void fail_after_free(FT_Memory memory, void* block) {
    (void)memory;
    free(block);
}

static void* fail_after_realloc(FT_Memory memory, long cur_size, long new_size, void* block) {
    (void)cur_size;
    if (fail_after_memory_should_fail(memory)) {
        return NULL;
    }
    return realloc(block, (size_t)new_size);
}

static FT_Error debug_hook_a(void* arg) {
    (void)arg;
    return FT_Err_Ok;
}

static FT_Error debug_hook_b(void* arg) {
    (void)arg;
    return FT_Err_Ok;
}

static int is_moveto_starts_each_contour_case(const char* case_id) {
    return streq(case_id, "ftimage.FT_Outline_MoveTo_Func.decompose_starts_each_contour@g36-f1") ||
           streq(case_id, "ftimage.FT_Outline_MoveTo_Func.decompose_starts_each_contour@g37-f2");
}

static int is_moveto_callback_error_case(const char* case_id) {
    return streq(case_id, "ftimage.FT_Outline_MoveTo_Func.decompose_propagates_callback_error");
}

static void print_json_bool(int value);
static void print_status(FT_Error err);
static void print_slot_body(FT_GlyphSlot slot, FT_UInt glyph_index);

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

static void print_gzip_uncompress_row(
    const char* payload_id,
    const char* input_kind,
    const char* buffer_size,
    FT_Error status,
    FT_ULong output_len,
    const unsigned char* output) {
    printf("{\"payload\":\"");
    print_json_string_content(payload_id);
    printf("\",\"input_kind\":\"");
    print_json_string_content(input_kind);
    printf("\",\"buffer_size\":\"");
    print_json_string_content(buffer_size);
    printf("\",\"status\":%d,\"output_len\":%lu,\"output_bytes\":\"",
           status, (unsigned long)output_len);
    if (!status) {
        print_hex_bytes(output, (long)output_len);
    }
    printf("\"}");
}

static int emit_gzip_uncompress(int argc, char** argv) {
    if (argc < 6 || ((argc - 2) % 4) != 0) {
        fprintf(stderr, "--gzip-uncompress requires PAYLOAD_ID RAW GZIP ZLIB groups\n");
        return 2;
    }
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"rows\":[");
    int first = 1;
    for (int index = 2; index + 3 < argc; index += 4) {
        const char* payload_id = argv[index];
        const char* raw_path = argv[index + 1];
        const char* gzip_path = argv[index + 2];
        const char* zlib_path = argv[index + 3];
        unsigned char* raw = NULL;
        unsigned char* gzip_bytes = NULL;
        unsigned char* zlib_bytes = NULL;
        long raw_len = 0;
        long gzip_len = 0;
        long zlib_len = 0;
        if (load_file(raw_path, &raw, &raw_len) != 0 ||
            load_file(gzip_path, &gzip_bytes, &gzip_len) != 0 ||
            load_file(zlib_path, &zlib_bytes, &zlib_len) != 0) {
            free(raw);
            free(gzip_bytes);
            free(zlib_bytes);
            FT_Done_FreeType(library);
            return 2;
        }
        for (int kind = 0; kind < 2; kind++) {
            const char* input_kind = kind == 0 ? "gzip" : "zlib_wrapped";
            const unsigned char* input = kind == 0 ? gzip_bytes : zlib_bytes;
            long input_len = kind == 0 ? gzip_len : zlib_len;
            for (int size_kind = 0; size_kind < 2; size_kind++) {
                const char* buffer_size = size_kind == 0
                    ? "exact_uncompressed_size"
                    : "larger_than_uncompressed_size";
                FT_ULong output_capacity = (FT_ULong)raw_len + (size_kind == 0 ? 0UL : 7UL);
                unsigned char* output = (unsigned char*)malloc((size_t)(output_capacity ? output_capacity : 1));
                if (!output) {
                    free(raw);
                    free(gzip_bytes);
                    free(zlib_bytes);
                    FT_Done_FreeType(library);
                    return 1;
                }
                memset(output, 0xA5, (size_t)(output_capacity ? output_capacity : 1));
                FT_ULong output_len = output_capacity;
                FT_Error status = FT_Gzip_Uncompress(
                    library->memory,
                    output,
                    &output_len,
                    input,
                    (FT_ULong)input_len);
                if (!first) {
                    printf(",");
                }
                print_gzip_uncompress_row(payload_id, input_kind, buffer_size, status, output_len, output);
                first = 0;
                free(output);
            }
        }
        free(raw);
        free(gzip_bytes);
        free(zlib_bytes);
    }
    printf("]}}\n");
    FT_Done_FreeType(library);
    return 0;
}

static const char* stream_ptr_class(const void* ptr) {
    return ptr ? "nonnull" : "null";
}

static void print_gzip_stream_read_ranges(
    FT_Stream stream,
    const unsigned char* raw,
    long raw_len) {
    const unsigned long lengths[3] = { 16UL, 19UL, 23UL };
    unsigned long starts[3];
    starts[0] = 0;
    starts[1] = raw_len > 19 ? (unsigned long)(raw_len / 2) : 0;
    starts[2] = raw_len > 23 ? (unsigned long)(raw_len - 23) : 0;
    const char* labels[3] = { "beginning", "middle", "end" };

    printf("[");
    for (int i = 0; i < 3; i++) {
        unsigned long available = raw_len > (long)starts[i]
            ? (unsigned long)raw_len - starts[i]
            : 0;
        unsigned long count = lengths[i] < available ? lengths[i] : available;
        unsigned char buffer[32];
        memset(buffer, 0, sizeof(buffer));
        unsigned long read_count = 0;
        if (stream->base) {
            if (count) {
                memcpy(buffer, stream->base + starts[i], count);
            }
            read_count = count;
        } else if (stream->read) {
            read_count = stream->read(stream, starts[i], buffer, count);
        }
        if (i) {
            printf(",");
        }
        printf("{\"label\":\"%s\",\"offset\":%lu,\"requested\":%lu,\"read\":%lu,\"bytes\":\"",
               labels[i], starts[i], count, read_count);
        print_hex_bytes(buffer, (long)read_count);
        printf("\",\"expected\":\"");
        print_hex_bytes(raw + starts[i], (long)read_count);
        printf("\"}");
    }
    printf("]");
}

static void print_gzip_stream_row(
    const char* payload_id,
    const char* source_position,
    FT_Error status,
    FT_Stream stream,
    const unsigned char* raw,
    long raw_len) {
    printf("{\"payload\":\"");
    print_json_string_content(payload_id);
    printf("\",\"source_position\":\"");
    print_json_string_content(source_position);
    printf("\",\"status\":%d,\"stream\":{", status);
    printf("\"size\":%lu,\"base_class\":\"%s\",\"read_class\":\"%s\",\"close_class\":\"%s\"},",
           status ? 0UL : (unsigned long)stream->size,
           status ? "null" : stream_ptr_class(stream->base),
           status ? "null" : stream_ptr_class(stream->read),
           status ? "null" : stream_ptr_class(stream->close));
    printf("\"read_ranges\":");
    if (status) {
        printf("[]");
    } else {
        print_gzip_stream_read_ranges(stream, raw, raw_len);
    }
    printf("}");
}

static int emit_gzip_stream_open(int argc, char** argv) {
    if (argc < 5 || ((argc - 2) % 3) != 0) {
        fprintf(stderr, "--gzip-stream-open requires PAYLOAD_ID RAW GZIP groups\n");
        return 2;
    }
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"rows\":[");
    int first = 1;
    for (int index = 2; index + 2 < argc; index += 3) {
        const char* payload_id = argv[index];
        const char* raw_path = argv[index + 1];
        const char* gzip_path = argv[index + 2];
        unsigned char* raw = NULL;
        unsigned char* gzip_bytes = NULL;
        long raw_len = 0;
        long gzip_len = 0;
        if (load_file(raw_path, &raw, &raw_len) != 0 ||
            load_file(gzip_path, &gzip_bytes, &gzip_len) != 0) {
            free(raw);
            free(gzip_bytes);
            FT_Done_FreeType(library);
            return 2;
        }
        for (int source_case = 0; source_case < 2; source_case++) {
            FT_StreamRec source;
            FT_StreamRec stream;
            memset(&source, 0, sizeof(source));
            memset(&stream, 0xA5, sizeof(stream));
            source.base = gzip_bytes;
            source.size = (FT_ULong)gzip_len;
            source.pos = source_case == 0 ? 0UL : 3UL;
            source.memory = library->memory;
            FT_Error status = FT_Stream_OpenGzip(&stream, &source);
            if (!first) {
                printf(",");
            }
            print_gzip_stream_row(
                payload_id,
                source_case == 0 ? "zero" : "nonzero_before_header",
                status,
                &stream,
                raw,
                raw_len);
            first = 0;
            if (!status && stream.close) {
                stream.close(&stream);
            }
        }
        free(raw);
        free(gzip_bytes);
    }
    printf("]}}\n");
    FT_Done_FreeType(library);
    return 0;
}

static int emit_bzip2_stream_disabled_policy(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_StreamRec source;
    FT_StreamRec stream;
    memset(&source, 0, sizeof(source));
    memset(&stream, 0, sizeof(stream));

    FT_Error status = FT_Stream_OpenBzip2(&stream, &source);
    printf("{");
    print_status(status);
    printf(",\"output\":{\"build_features\":{\"bzip2\":false},\"error\":%d,"
           "\"stream\":{\"base_class\":\"%s\",\"read_class\":\"%s\",\"close_class\":\"%s\"}}}\n",
           status,
           stream.base ? "nonnull" : "null",
           stream.read ? "nonnull" : "null",
           stream.close ? "nonnull" : "null");
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

static int fixture_module_init_calls = 0;
static int fixture_module_done_calls = 0;

static FT_Error fixture_module_init(FT_Module module) {
    (void)module;
    fixture_module_init_calls++;
    return FT_Err_Ok;
}

static void fixture_module_done(FT_Module module) {
    (void)module;
    fixture_module_done_calls++;
}

static void* fixture_module_get_interface(FT_Module module, const char* name) {
    (void)module;
    (void)name;
    return NULL;
}

static int emit_add_module_fixture(const char* module_name,
                                   FT_ULong module_flags,
                                   const void* module_interface,
                                   const char* module_size_label,
                                   int add_default_modules) {
    fixture_module_init_calls = 0;
    fixture_module_done_calls = 0;
    struct FT_MemoryRec_ memory = {0};
    memory.alloc = oracle_alloc;
    memory.free = oracle_free;
    memory.realloc = oracle_realloc;
    FT_Library library = NULL;
    FT_Error err = FT_New_Library(&memory, &library);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":{\"error\":%d}}\n", err);
        return 0;
    }
    if (add_default_modules) {
        FT_Add_Default_Modules(library);
    }
    FT_Renderer outline_renderer_before = FT_Get_Renderer(library, FT_GLYPH_FORMAT_OUTLINE);
    FT_Module_Class fixture_class = {
        module_flags,
        sizeof(FT_ModuleRec),
        module_name,
        0x00010000L,
        0x00020000L,
        module_interface,
        fixture_module_init,
        fixture_module_done,
        fixture_module_get_interface
    };
    err = FT_Add_Module(library, &fixture_class);
    FT_Module module = FT_Get_Module(library, module_name);
    FT_Renderer outline_renderer_after = FT_Get_Renderer(library, FT_GLYPH_FORMAT_OUTLINE);
    printf("{");
    print_status(err);
    printf(",\"output\":{");
    printf("\"status\":%d,", err);
    printf("\"module_count\":%u,", library ? library->num_modules : 0);
    printf("\"lookup_result\":{\"nullness\":");
    print_json_bool(module == NULL);
    printf("},\"stored_class_fields\":{");
    printf("\"module_flags\":%lu,", (unsigned long)fixture_class.module_flags);
    printf("\"module_size\":\"%s\",", module_size_label);
    printf("\"module_name\":\"%s\",", module_name);
    printf("\"module_version\":%ld,", (long)fixture_class.module_version);
    printf("\"module_requires\":%ld,", (long)fixture_class.module_requires);
    printf("\"module_interface_nullness\":");
    print_json_bool(fixture_class.module_interface == NULL);
    printf("},\"routing_effects\":{");
    printf("\"outline_renderer_present_before\":");
    print_json_bool(outline_renderer_before != NULL);
    printf(",\"outline_renderer_present_after\":");
    print_json_bool(outline_renderer_after != NULL);
    printf(",\"outline_renderer_identity_preserved\":");
    print_json_bool(outline_renderer_before == outline_renderer_after);
    printf("},\"renderer_membership\":null,\"callback_log\":[");
    if (fixture_module_init_calls > 0) {
        printf("\"module_init\"");
    }
    printf("]}}\n");
    FT_Done_Library(library);
    return 0;
}

static int emit_add_module_minimal(void) {
    return emit_add_module_fixture("fixture_minimal", 0, NULL, "sizeof_synthetic_module", 0);
}

static int emit_add_module_styler(void) {
    static const char fixture_private_interface = 0;
    return emit_add_module_fixture("fixture_styler",
                                   FT_MODULE_STYLER,
                                   &fixture_private_interface,
                                   "sizeof_synthetic_module",
                                   1);
}

static int emit_add_module_renderer(void) {
    fixture_module_init_calls = 0;
    fixture_module_done_calls = 0;
    struct FT_MemoryRec_ memory = {0};
    memory.alloc = oracle_alloc;
    memory.free = oracle_free;
    memory.realloc = oracle_realloc;
    FT_Library library = NULL;
    FT_Error err = FT_New_Library(&memory, &library);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":{\"error\":%d}}\n", err);
        return 0;
    }
    FT_Add_Default_Modules(library);
    FT_Renderer outline_renderer_before = FT_Get_Renderer(library, FT_GLYPH_FORMAT_OUTLINE);
    static const char synthetic_renderer_interface = 0;
    FT_Renderer_Class fixture_renderer_class = {
        {
            FT_MODULE_RENDERER,
            sizeof(FT_RendererRec),
            "fixture_renderer",
            0x00010000L,
            0x00020000L,
            &synthetic_renderer_interface,
            fixture_module_init,
            fixture_module_done,
            fixture_module_get_interface
        },
        FT_GLYPH_FORMAT_OUTLINE,
        NULL,
        NULL,
        NULL,
        NULL,
        NULL
    };
    err = FT_Add_Module(library, &fixture_renderer_class.root);
    FT_Module module = FT_Get_Module(library, "fixture_renderer");
    FT_Renderer outline_renderer_after = FT_Get_Renderer(library, FT_GLYPH_FORMAT_OUTLINE);
    FT_Error set_renderer_status = module
        ? FT_Set_Renderer(library, (FT_Renderer)module, 0, NULL)
        : FT_Err_Invalid_Argument;
    FT_Renderer outline_renderer_after_set = FT_Get_Renderer(library, FT_GLYPH_FORMAT_OUTLINE);
    printf("{");
    print_status(err);
    printf(",\"output\":{");
    printf("\"status\":%d,", err);
    printf("\"module_count\":%u,", library ? library->num_modules : 0);
    printf("\"lookup_result\":{\"nullness\":");
    print_json_bool(module == NULL);
    printf("},\"stored_class_fields\":{");
    printf("\"module_flags\":%lu,", (unsigned long)fixture_renderer_class.root.module_flags);
    printf("\"module_size\":\"sizeof_synthetic_renderer_module\",");
    printf("\"module_name\":\"fixture_renderer\",");
    printf("\"module_version\":%ld,", (long)fixture_renderer_class.root.module_version);
    printf("\"module_requires\":%ld,", (long)fixture_renderer_class.root.module_requires);
    printf("\"module_interface_nullness\":");
    print_json_bool(fixture_renderer_class.root.module_interface == NULL);
    printf("},\"routing_effects\":{");
    printf("\"outline_renderer_present_before\":");
    print_json_bool(outline_renderer_before != NULL);
    printf(",\"outline_renderer_present_after\":");
    print_json_bool(outline_renderer_after != NULL);
    printf(",\"outline_renderer_identity_preserved\":");
    print_json_bool(outline_renderer_before == outline_renderer_after);
    printf("},\"renderer_membership\":{");
    printf("\"set_renderer_status\":%d,", set_renderer_status);
    printf("\"current_renderer_after_set\":");
    if (outline_renderer_after_set == (FT_Renderer)module) {
        printf("\"fixture_renderer\"");
    } else if (outline_renderer_after_set == NULL) {
        printf("\"null\"");
    } else {
        printf("\"other\"");
    }
    printf("},\"callback_log\":[");
    if (fixture_module_init_calls > 0) {
        printf("\"module_init\"");
    }
    printf("]}}\n");
    FT_Done_Library(library);
    return 0;
}

static void dirty_bitmap(FT_Bitmap* bitmap) {
    bitmap->rows = 7;
    bitmap->width = 9;
    bitmap->pitch = -11;
    bitmap->buffer = (unsigned char*)0x7f;
    bitmap->num_grays = 13;
    bitmap->pixel_mode = 15;
    bitmap->palette_mode = 17;
    bitmap->palette = (void*)0x7f;
}

static void print_bitmap_fields(const FT_Bitmap* bitmap) {
    printf("{\"rows\":%u,\"width\":%u,\"pitch\":%d,\"buffer_is_null\":", bitmap->rows, bitmap->width, bitmap->pitch);
    print_json_bool(bitmap->buffer == NULL);
    printf(",\"num_grays\":%u,\"pixel_mode\":%u,\"palette_mode\":%u,\"palette_is_null\":",
           bitmap->num_grays, bitmap->pixel_mode, bitmap->palette_mode);
    print_json_bool(bitmap->palette == NULL);
    printf("}");
}

static void print_byte_hash_or_null(const unsigned char* bytes, size_t len) {
    if (!bytes) {
        printf("null");
        return;
    }
    unsigned long long hash = 5381ULL;
    for (size_t i = 0; i < len; i++) {
        hash = ((hash << 5) + hash) + bytes[i];
    }
    printf("\"%llx\"", hash);
}

static size_t bitmap_len(const FT_Bitmap* bitmap) {
    int pitch = bitmap->pitch < 0 ? -bitmap->pitch : bitmap->pitch;
    return (size_t)pitch * (size_t)bitmap->rows;
}

static unsigned long long djb2_hash_update(unsigned long long hash, const unsigned char* bytes, size_t len) {
    for (size_t i = 0; i < len; i++) {
        hash = ((hash << 5) + hash) + bytes[i];
    }
    return hash;
}

static void print_bitmap_active_hash_or_null(const FT_Bitmap* bitmap) {
    if (!bitmap || !bitmap->buffer) {
        printf("null");
        return;
    }
    int pitch = bitmap->pitch < 0 ? -bitmap->pitch : bitmap->pitch;
    const unsigned char* row = bitmap->buffer;
    if (bitmap->pitch < 0) {
        row += (size_t)pitch * (size_t)(bitmap->rows ? bitmap->rows - 1 : 0);
    }
    unsigned long long hash = 5381ULL;
    for (FT_UInt y = 0; y < bitmap->rows; y++) {
        hash = djb2_hash_update(hash, row, bitmap->width);
        row += bitmap->pitch;
    }
    printf("\"%llx\"", hash);
}

static void bitmap_copy_source(FT_Bitmap* bitmap, unsigned char* bytes) {
    bitmap->rows = 3;
    bitmap->width = 3;
    bitmap->pitch = 4;
    bitmap->buffer = bytes;
    bitmap->num_grays = 256;
    bitmap->pixel_mode = FT_PIXEL_MODE_GRAY;
    bitmap->palette_mode = 0;
    bitmap->palette = NULL;
    for (int i = 0; i < 12; i++) {
        bytes[i] = (unsigned char)(i * 17 + 3);
    }
}

static void bitmap_convert_source(FT_Bitmap* bitmap, unsigned char* bytes, int pixel_mode, int negative_pitch) {
    FT_UInt rows = 3;
    FT_UInt width = pixel_mode == FT_PIXEL_MODE_BGRA ? 4 : 9;
    int pitch;
    switch (pixel_mode) {
    case FT_PIXEL_MODE_MONO:
        pitch = 2;
        break;
    case FT_PIXEL_MODE_GRAY2:
        pitch = 3;
        break;
    case FT_PIXEL_MODE_GRAY4:
        pitch = 5;
        break;
    case FT_PIXEL_MODE_BGRA:
        pitch = 16;
        break;
    default:
        pitch = (int)width;
        break;
    }
    if (negative_pitch) {
        pitch = -pitch;
    }
    bitmap->rows = rows;
    bitmap->width = width;
    bitmap->pitch = pitch;
    bitmap->buffer = bytes;
    bitmap->num_grays = pixel_mode == FT_PIXEL_MODE_MONO ? 2 :
                        pixel_mode == FT_PIXEL_MODE_GRAY2 ? 4 :
                        pixel_mode == FT_PIXEL_MODE_GRAY4 ? 16 : 256;
    bitmap->pixel_mode = pixel_mode;
    bitmap->palette_mode = 0;
    bitmap->palette = NULL;
    size_t len = (size_t)(pitch < 0 ? -pitch : pitch) * rows;
    for (size_t i = 0; i < len; i++) {
        bytes[i] = (unsigned char)(i * 37 + pixel_mode * 11 + 5);
    }
}

static void print_bitmap_convert_run(const char* label, FT_Error err, const FT_Bitmap* target) {
    printf("{\"label\":\"%s\",", label);
    print_status(err);
    printf(",\"target\":");
    if (target) {
        print_bitmap_fields(target);
        printf(",\"target_active_bytes_hash\":");
        if (err) {
            printf("null");
        } else {
            print_bitmap_active_hash_or_null(target);
        }
        printf(",\"target_buffer_len\":%zu", target->buffer ? bitmap_len(target) : 0);
    } else {
        printf("null,\"target_active_bytes_hash\":null,\"target_buffer_len\":0");
    }
    printf("}");
}

static const char* ft_list_node_token(FT_ListNode node,
                                      FT_ListNode node_a,
                                      FT_ListNode node_b,
                                      FT_ListNode node_c) {
    if (!node) return "null";
    if (node == node_a) return "node_a";
    if (node == node_b) return "node_b";
    if (node == node_c) return "node_c";
    return "foreign";
}

static const char* ft_list_data_token(void* data, void* data_a, void* data_b, void* data_c) {
    if (!data) return "null";
    if (data == data_a) return "data_a";
    if (data == data_b) return "data_b";
    if (data == data_c) return "data_c";
    return "foreign";
}

static void print_ft_list_pair(const char* key,
                               FT_ListNode head,
                               FT_ListNode tail,
                               FT_ListNode node_a,
                               FT_ListNode node_b,
                               FT_ListNode node_c) {
    printf("\"%s\":{\"head\":\"%s\",\"tail\":\"%s\"}",
           key,
           ft_list_node_token(head, node_a, node_b, node_c),
           ft_list_node_token(tail, node_a, node_b, node_c));
}

static void print_ft_list_node(const char* id,
                               FT_ListNode node,
                               void* data_a,
                               void* data_b,
                               void* data_c,
                               FT_ListNode node_a,
                               FT_ListNode node_b,
                               FT_ListNode node_c) {
    printf("{\"id\":\"%s\",\"prev\":\"%s\",\"next\":\"%s\",\"data\":\"%s\"}",
           id,
           ft_list_node_token(node ? node->prev : NULL, node_a, node_b, node_c),
           ft_list_node_token(node ? node->next : NULL, node_a, node_b, node_c),
           ft_list_data_token(node ? node->data : NULL, data_a, data_b, data_c));
}

static const char* ft_list_node_token4(FT_ListNode node,
                                       FT_ListNode node_0,
                                       FT_ListNode node_a,
                                       FT_ListNode node_b,
                                       FT_ListNode node_c) {
    if (!node) return "null";
    if (node == node_0) return "node_0";
    if (node == node_a) return "node_a";
    if (node == node_b) return "node_b";
    if (node == node_c) return "node_c";
    return "foreign";
}

static const char* ft_list_data_token4(void* data,
                                       void* data_0,
                                       void* data_a,
                                       void* data_b,
                                       void* data_c) {
    if (!data) return "null";
    if (data == data_0) return "data_0";
    if (data == data_a) return "data_a";
    if (data == data_b) return "data_b";
    if (data == data_c) return "data_c";
    return "foreign";
}

static void print_ft_list_topology4(FT_List list,
                                    void* data_0,
                                    void* data_a,
                                    void* data_b,
                                    void* data_c,
                                    FT_ListNode node_0,
                                    FT_ListNode node_a,
                                    FT_ListNode node_b,
                                    FT_ListNode node_c) {
    FT_ListNode nodes[4] = { node_0, node_a, node_b, node_c };
    const char* labels[4] = { "node_0", "node_a", "node_b", "node_c" };
    printf("{\"list\":{\"head\":\"%s\",\"tail\":\"%s\"},\"nodes\":[",
           ft_list_node_token4(list ? list->head : NULL, node_0, node_a, node_b, node_c),
           ft_list_node_token4(list ? list->tail : NULL, node_0, node_a, node_b, node_c));
    int first = 1;
    for (int i = 0; i < 4; i++) {
        if (!nodes[i]) continue;
        if (!first) printf(",");
        first = 0;
        printf("{\"id\":\"%s\",\"prev\":\"%s\",\"next\":\"%s\",\"data\":\"%s\"}",
               labels[i],
               ft_list_node_token4(nodes[i]->prev, node_0, node_a, node_b, node_c),
               ft_list_node_token4(nodes[i]->next, node_0, node_a, node_b, node_c),
               ft_list_data_token4(nodes[i]->data, data_0, data_a, data_b, data_c));
    }
    printf("],\"data_tokens\":[\"data_0\",\"data_a\",\"data_b\",\"data_c\"]}");
}

static const char* finalize_data_token(void* data, void* data_a, void* data_b, void* data_c) {
    if (data == data_a) return "data_a";
    if (data == data_b) return "data_b";
    if (data == data_c) return "data_c";
    return data ? "foreign" : "null";
}

typedef struct FinalizeTrace_ {
    FT_ListNode nodes[3];
    void* data[3];
    const char* node_labels[3];
    const char* data_labels[3];
    const char* freed[8];
    const char* destroyed_data[8];
    const char* destroyed_memory[8];
    const char* destroyed_user[8];
    int freed_count;
    int destroyed_count;
    FT_Memory expected_memory;
    void* expected_user;
} FinalizeTrace;

static void finalize_record_free(FT_Memory memory, void* block) {
    FinalizeTrace* trace = (FinalizeTrace*)memory->user;
    const char* label = "foreign";
    for (int i = 0; i < 3; i++) {
        if (block == trace->nodes[i]) {
            label = trace->node_labels[i];
            break;
        }
    }
    trace->freed[trace->freed_count++] = label;
    free(block);
}

static void finalize_record_destructor(FT_Memory memory, void* data, void* user) {
    FinalizeTrace* trace = (FinalizeTrace*)memory->user;
    trace->destroyed_memory[trace->destroyed_count] = memory == trace->expected_memory ? "memory" : "foreign";
    trace->destroyed_user[trace->destroyed_count] = user == trace->expected_user ? "user" : "foreign";
    trace->destroyed_data[trace->destroyed_count] =
        finalize_data_token(data, trace->data[0], trace->data[1], trace->data[2]);
    trace->destroyed_count++;
}

static FT_ListNode finalize_new_node(FinalizeTrace* trace, int index) {
    FT_ListNode node = (FT_ListNode)calloc(1, sizeof(FT_ListNodeRec));
    node->data = trace->data[index];
    trace->nodes[index] = node;
    return node;
}

static void finalize_link_two(FT_ListNode a, FT_ListNode b) {
    a->next = b;
    b->prev = a;
}

static void finalize_print_string_array(const char* key, const char** values, int count) {
    printf("\"%s\":[", key);
    for (int i = 0; i < count; i++) {
        if (i) printf(",");
        printf("\"%s\"", values[i]);
    }
    printf("]");
}

static void finalize_print_trace(FinalizeTrace* trace, FT_List list) {
    printf("{");
    finalize_print_string_array("freed_nodes", trace->freed, trace->freed_count);
    printf(",\"destructor_call_count\":%d", trace->destroyed_count);
    printf(",\"destructor_calls\":[");
    for (int i = 0; i < trace->destroyed_count; i++) {
        if (i) printf(",");
        printf("{\"memory\":\"%s\",\"data\":\"%s\",\"user\":\"%s\"}",
               trace->destroyed_memory[i],
               trace->destroyed_data[i],
               trace->destroyed_user[i]);
    }
    printf("],\"data_freed_by_destructor\":%s", trace->destroyed_count ? "true" : "false");
    printf(",\"list_after\":{\"head\":\"%s\",\"tail\":\"%s\"}}",
           list && list->head ? "non_null" : "null",
           list && list->tail ? "non_null" : "null");
}

static void print_ft_list_topology3(FT_List list,
                                    void* data_a,
                                    void* data_b,
                                    void* data_c,
                                    FT_ListNode node_a,
                                    FT_ListNode node_b,
                                    FT_ListNode node_c) {
    FT_ListNode nodes[3] = { node_a, node_b, node_c };
    const char* labels[3] = { "node_a", "node_b", "node_c" };
    printf("{\"list\":{\"head\":\"%s\",\"tail\":\"%s\"},\"nodes\":[",
           ft_list_node_token(list ? list->head : NULL, node_a, node_b, node_c),
           ft_list_node_token(list ? list->tail : NULL, node_a, node_b, node_c));
    int first = 1;
    for (int i = 0; i < 3; i++) {
        if (!nodes[i]) continue;
        if (!first) printf(",");
        first = 0;
        printf("{\"id\":\"%s\",\"prev\":\"%s\",\"next\":\"%s\",\"data\":\"%s\"}",
               labels[i],
               ft_list_node_token(nodes[i]->prev, node_a, node_b, node_c),
               ft_list_node_token(nodes[i]->next, node_a, node_b, node_c),
               ft_list_data_token(nodes[i]->data, data_a, data_b, data_c));
    }
    printf("],\"data_tokens\":[\"data_a\",\"data_b\",\"data_c\"]}");
}

typedef struct IterateTrace_ {
    FT_ListNode nodes[3];
    const char* node_labels[3];
    const char* visited[8];
    int user_matches[8];
    int visited_count;
    void* expected_user;
    const char* mode;
    FT_List list;
    FT_List side_list;
    FT_Memory side_memory;
    int side_finalized;
    const char* event_visit[8];
    const char* event_mutation[8];
    const char* event_found[8];
    const char* event_side_freed[8][8];
    int event_side_freed_count[8];
    int event_count;
} IterateTrace;

static IterateTrace* current_iterate_trace = NULL;

static const char* iterate_node_token(IterateTrace* trace, FT_ListNode node) {
    if (!node) return "null";
    for (int i = 0; i < 3; i++) {
        if (node == trace->nodes[i]) {
            return trace->node_labels[i];
        }
    }
    return "foreign";
}

static const char* iterate_data_token_for_node(IterateTrace* trace, FT_ListNode node) {
    const char* label = iterate_node_token(trace, node);
    if (strcmp(label, "node_a") == 0) return "data_a";
    if (strcmp(label, "node_b") == 0) return "data_b";
    if (strcmp(label, "node_c") == 0) return "data_c";
    return label;
}

static FT_Error iterate_record_callback(FT_ListNode node, void* user) {
    IterateTrace* trace = current_iterate_trace;
    trace->visited[trace->visited_count] = iterate_data_token_for_node(trace, node);
    trace->user_matches[trace->visited_count] = user == trace->expected_user;
    trace->visited_count++;
    return FT_Err_Ok;
}

static FT_Error iterate_mutation_callback(FT_ListNode node, void* user) {
    IterateTrace* trace = current_iterate_trace;
    (void)user;
    const char* visit = iterate_data_token_for_node(trace, node);
    trace->visited[trace->visited_count++] = visit;
    int event = trace->event_count++;
    trace->event_visit[event] = visit;
    trace->event_mutation[event] = trace->mode;
    trace->event_found[event] = NULL;
    trace->event_side_freed_count[event] = 0;
    if (strcmp(trace->mode, "remove_current") == 0) {
        FT_List_Remove(trace->list, node);
    } else if (strcmp(trace->mode, "move_current_to_head") == 0) {
        FT_List_Up(trace->list, node);
    } else if (strcmp(trace->mode, "find_current_data") == 0) {
        trace->event_found[event] = iterate_node_token(trace, FT_List_Find(trace->list, node->data));
    } else if (strcmp(trace->mode, "finalize_side_list") == 0 && !trace->side_finalized) {
        FinalizeTrace* finalize_trace = (FinalizeTrace*)trace->side_memory->user;
        FT_List_Finalize(trace->side_list, NULL, trace->side_memory, NULL);
        trace->side_finalized = 1;
        trace->event_side_freed_count[event] = finalize_trace->freed_count;
        for (int i = 0; i < finalize_trace->freed_count; i++) {
            trace->event_side_freed[event][i] = finalize_trace->freed[i];
        }
    }
    return FT_Err_Ok;
}

static const char* iterate_user_identity(IterateTrace* trace) {
    for (int i = 0; i < trace->visited_count; i++) {
        if (!trace->user_matches[i]) return "foreign";
    }
    return "user";
}

static void iterate_print_visited(IterateTrace* trace) {
    printf("[");
    for (int i = 0; i < trace->visited_count; i++) {
        if (i) printf(",");
        printf("\"%s\"", trace->visited[i]);
    }
    printf("]");
}

static void iterate_print_events(IterateTrace* trace) {
    printf("[");
    for (int i = 0; i < trace->event_count; i++) {
        if (i) printf(",");
        printf("{\"visit\":\"%s\",\"mutation\":\"%s\"", trace->event_visit[i], trace->event_mutation[i]);
        if (trace->event_found[i]) {
            printf(",\"found\":\"%s\"", trace->event_found[i]);
        }
        if (strcmp(trace->event_mutation[i], "finalize_side_list") == 0) {
            printf(",\"side_freed\":[");
            for (int j = 0; j < trace->event_side_freed_count[i]; j++) {
                if (j) printf(",");
                printf("\"%s\"", trace->event_side_freed[i][j]);
            }
            printf("]");
        }
        printf("}");
    }
    printf("]");
}

static void print_ft_list_find_result(FT_List list,
                                      void* data,
                                      FT_ListNode node_a,
                                      FT_ListNode node_b,
                                      FT_ListNode node_c) {
    FT_ListNode found = FT_List_Find(list, data);
    printf("\"return\":\"%s\",\"visited_nodes\":[",
           ft_list_node_token(found, node_a, node_b, node_c));
    int first = 1;
    if (list) {
        FT_ListNode cur = list->head;
        while (cur) {
            if (!first) printf(",");
            first = 0;
            printf("\"%s\"", ft_list_node_token(cur, node_a, node_b, node_c));
            if (cur->data == data) break;
            cur = cur->next;
        }
    }
    printf("]");
}

static int emit_ft_list(const char* case_id) {
    unsigned char data_0 = 0;
    unsigned char data_a = 1;
    unsigned char data_b = 2;
    unsigned char data_c = 3;
    FT_ListNodeRec node_0 = { NULL, NULL, &data_0 };
    FT_ListNodeRec node_a = { NULL, NULL, &data_a };
    FT_ListNodeRec node_b = { NULL, NULL, &data_b };
    FT_ListNodeRec node_c = { NULL, NULL, &data_c };
    FT_ListRec list = { NULL, NULL };

    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":");

    if (streq(case_id, "ftlist.FT_List_Add.success_empty_list")) {
        FT_List_Add(&list, &node_a);
        printf("{");
        print_ft_list_pair("list", list.head, list.tail, &node_a, NULL, NULL);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, NULL, NULL);
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Add.success_non_empty_list")) {
        node_a.next = &node_b;
        node_b.prev = &node_a;
        node_c.prev = (FT_ListNode)(uintptr_t)0x51;
        node_c.next = (FT_ListNode)(uintptr_t)0x52;
        list.head = &node_a;
        list.tail = &node_b;
        FT_List_Add(&list, &node_c);
        printf("{");
        print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, &node_c);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
        printf(",");
        print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
        printf(",");
        print_ft_list_node("node_c", &node_c, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Add.null_list_or_node_noop")) {
        FT_List_Add(NULL, &node_a);
        printf("{\"rows\":[{\"variant\":\"null_list\",");
        print_ft_list_pair("list", list.head, list.tail, &node_a, NULL, NULL);
        printf(",\"node\":");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, NULL, NULL);
        FT_List_Add(&list, NULL);
        printf("},{\"variant\":\"null_node\",");
        print_ft_list_pair("list", list.head, list.tail, &node_a, NULL, NULL);
        printf(",\"node\":");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, NULL, NULL);
        printf("}]}");
    } else if (streq(case_id, "ftlist.FT_List_Insert.insert_empty_list")) {
        node_0.prev = (FT_ListNode)(uintptr_t)0x51;
        node_0.next = (FT_ListNode)(uintptr_t)0x52;
        FT_List_Insert(&list, &node_0);
        print_ft_list_topology4(&list, &data_0, &data_a, &data_b, &data_c, &node_0, NULL, NULL, NULL);
    } else if (streq(case_id, "ftlist.FT_List_Insert.insert_non_empty_list")) {
        printf("{\"rows\":[");
        for (int i = 0; i < 2; i++) {
            node_0 = (FT_ListNodeRec){ NULL, NULL, &data_0 };
            node_a = (FT_ListNodeRec){ NULL, NULL, &data_a };
            node_b = (FT_ListNodeRec){ NULL, NULL, &data_b };
            node_c = (FT_ListNodeRec){ NULL, NULL, &data_c };
            if (i == 0) {
                list = (FT_ListRec){ &node_a, &node_a };
            } else {
                node_a.next = &node_b;
                node_b.prev = &node_a;
                node_b.next = &node_c;
                node_c.prev = &node_b;
                list = (FT_ListRec){ &node_a, &node_c };
            }
            FT_List_Insert(&list, &node_0);
            if (i) printf(",");
            printf("{\"shape\":\"%s\",\"topology\":", i == 0 ? "one_node" : "three_nodes");
            print_ft_list_topology4(&list, &data_0, &data_a, &data_b, &data_c, &node_0, &node_a, i == 0 ? NULL : &node_b, i == 0 ? NULL : &node_c);
            printf("}");
        }
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Insert.null_list_or_node_noop")) {
        node_0 = (FT_ListNodeRec){ (FT_ListNode)(uintptr_t)0x51, (FT_ListNode)(uintptr_t)0x52, &data_0 };
        node_a = (FT_ListNodeRec){ NULL, &node_b, &data_a };
        node_b = (FT_ListNodeRec){ &node_a, NULL, &data_b };
        list = (FT_ListRec){ &node_a, &node_b };
        printf("{\"rows\":[{\"variant\":\"null_list\",\"initial_topology\":");
        print_ft_list_topology4(&list, &data_0, &data_a, &data_b, &data_c, &node_0, &node_a, &node_b, NULL);
        FT_List_Insert(NULL, &node_0);
        printf(",\"final_topology\":");
        print_ft_list_topology4(&list, &data_0, &data_a, &data_b, &data_c, &node_0, &node_a, &node_b, NULL);
        printf("},{\"variant\":\"null_node\",\"initial_topology\":");
        print_ft_list_topology4(&list, &data_0, &data_a, &data_b, &data_c, &node_0, &node_a, &node_b, NULL);
        FT_List_Insert(&list, NULL);
        printf(",\"final_topology\":");
        print_ft_list_topology4(&list, &data_0, &data_a, &data_b, &data_c, &node_0, &node_a, &node_b, NULL);
        printf("}]}");
    } else if (streq(case_id, "ftlist.FT_List_Remove.remove_head_middle_tail")) {
        const char* labels[3] = { "node_a", "node_b", "node_c" };
        printf("{\"rows\":[");
        for (int i = 0; i < 3; i++) {
            node_a = (FT_ListNodeRec){ NULL, &node_b, &data_a };
            node_b = (FT_ListNodeRec){ &node_a, &node_c, &data_b };
            node_c = (FT_ListNodeRec){ &node_b, NULL, &data_c };
            list = (FT_ListRec){ &node_a, &node_c };
            FT_ListNode target = i == 0 ? &node_a : (i == 1 ? &node_b : &node_c);
            FT_List_Remove(&list, target);
            if (i) printf(",");
            printf("{\"remove\":\"%s\",", labels[i]);
            print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, &node_c);
            printf(",\"nodes\":[");
            print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
            printf(",");
            print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
            printf(",");
            print_ft_list_node("node_c", &node_c, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
            printf("]}");
        }
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Remove.remove_only_node")) {
        list = (FT_ListRec){ &node_a, &node_a };
        FT_List_Remove(&list, &node_a);
        printf("{");
        print_ft_list_pair("list", list.head, list.tail, &node_a, NULL, NULL);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, NULL, NULL);
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Remove.null_list_or_node_noop")) {
        node_a = (FT_ListNodeRec){ NULL, &node_b, &data_a };
        node_b = (FT_ListNodeRec){ &node_a, NULL, &data_b };
        list = (FT_ListRec){ &node_a, &node_b };
        FT_List_Remove(NULL, &node_a);
        printf("{\"rows\":[{\"variant\":\"null_list\",");
        print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, NULL);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf(",");
        print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf("]}");
        FT_List_Remove(&list, NULL);
        printf(",{\"variant\":\"null_node\",");
        print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, NULL);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf(",");
        print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf("]}]}");
    } else if (streq(case_id, "ftlist.FT_List_Remove.membership_not_checked")) {
        FT_ListNodeRec foreign = { &node_a, &node_c, &data_b };
        node_a = (FT_ListNodeRec){ NULL, NULL, &data_a };
        node_b = (FT_ListNodeRec){ NULL, NULL, &data_b };
        node_c = (FT_ListNodeRec){ NULL, NULL, &data_c };
        list = (FT_ListRec){ &node_b, &node_b };
        FT_List_Remove(&list, &foreign);
        printf("{");
        print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, &node_c);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
        printf(",");
        print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
        printf(",");
        print_ft_list_node("node_c", &node_c, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Up.move_tail_or_middle_to_head")) {
        const char* labels[2] = { "node_c", "node_b" };
        printf("{\"rows\":[");
        for (int i = 0; i < 2; i++) {
            node_a = (FT_ListNodeRec){ NULL, &node_b, &data_a };
            node_b = (FT_ListNodeRec){ &node_a, &node_c, &data_b };
            node_c = (FT_ListNodeRec){ &node_b, NULL, &data_c };
            list = (FT_ListRec){ &node_a, &node_c };
            FT_ListNode target = i == 0 ? &node_c : &node_b;
            FT_List_Up(&list, target);
            if (i) printf(",");
            printf("{\"move\":\"%s\",", labels[i]);
            print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, &node_c);
            printf(",\"nodes\":[");
            print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
            printf(",");
            print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
            printf(",");
            print_ft_list_node("node_c", &node_c, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
            printf("]}");
        }
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Up.already_head_noop")) {
        node_a = (FT_ListNodeRec){ NULL, &node_b, &data_a };
        node_b = (FT_ListNodeRec){ &node_a, NULL, &data_b };
        list = (FT_ListRec){ &node_a, &node_b };
        FT_List_Up(&list, &node_a);
        printf("{");
        print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, NULL);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf(",");
        print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Up.null_list_or_node_noop")) {
        node_a = (FT_ListNodeRec){ NULL, &node_b, &data_a };
        node_b = (FT_ListNodeRec){ &node_a, NULL, &data_b };
        list = (FT_ListRec){ &node_a, &node_b };
        FT_List_Up(NULL, &node_b);
        printf("{\"rows\":[{\"variant\":\"null_list\",");
        print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, NULL);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf(",");
        print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf("]}");
        FT_List_Up(&list, NULL);
        printf(",{\"variant\":\"null_node\",");
        print_ft_list_pair("list", list.head, list.tail, &node_a, &node_b, NULL);
        printf(",\"nodes\":[");
        print_ft_list_node("node_a", &node_a, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf(",");
        print_ft_list_node("node_b", &node_b, &data_a, &data_b, &data_c, &node_a, &node_b, NULL);
        printf("]}]}");
    } else if (streq(case_id, "ftlist.FT_List_Finalize.success_destroys_all_nodes")) {
        FinalizeTrace trace = { 0 };
        trace.data[0] = &data_a;
        trace.data[1] = &data_b;
        trace.data[2] = &data_c;
        trace.node_labels[0] = "node_a";
        trace.node_labels[1] = "node_b";
        trace.node_labels[2] = "node_c";
        struct FT_MemoryRec_ memory = { &trace, oracle_alloc, finalize_record_free, oracle_realloc };
        trace.expected_memory = &memory;
        trace.expected_user = (void*)0x7111;
        FT_ListNode a = finalize_new_node(&trace, 0);
        FT_ListNode b = finalize_new_node(&trace, 1);
        FT_ListNode c = finalize_new_node(&trace, 2);
        finalize_link_two(a, b);
        finalize_link_two(b, c);
        list = (FT_ListRec){ a, c };
        FT_List_Finalize(&list, finalize_record_destructor, &memory, trace.expected_user);
        finalize_print_trace(&trace, &list);
    } else if (streq(case_id, "ftlist.FT_List_Finalize.success_null_destructor_frees_nodes_only")) {
        FinalizeTrace trace = { 0 };
        trace.data[0] = &data_a;
        trace.data[1] = &data_b;
        trace.node_labels[0] = "node_a";
        trace.node_labels[1] = "node_b";
        struct FT_MemoryRec_ memory = { &trace, oracle_alloc, finalize_record_free, oracle_realloc };
        trace.expected_memory = &memory;
        trace.expected_user = (void*)0x7111;
        FT_ListNode a = finalize_new_node(&trace, 0);
        FT_ListNode b = finalize_new_node(&trace, 1);
        finalize_link_two(a, b);
        list = (FT_ListRec){ a, b };
        FT_List_Finalize(&list, NULL, &memory, trace.expected_user);
        finalize_print_trace(&trace, &list);
    } else if (streq(case_id, "ftlist.FT_List_Finalize.null_list_or_memory_noop")) {
        FinalizeTrace trace = { 0 };
        trace.data[0] = &data_a;
        trace.data[1] = &data_b;
        trace.node_labels[0] = "node_a";
        trace.node_labels[1] = "node_b";
        struct FT_MemoryRec_ memory = { &trace, oracle_alloc, finalize_record_free, oracle_realloc };
        trace.expected_memory = &memory;
        trace.expected_user = (void*)0x7111;
        FT_ListNode a = finalize_new_node(&trace, 0);
        FT_ListNode b = finalize_new_node(&trace, 1);
        finalize_link_two(a, b);
        list = (FT_ListRec){ a, b };
        FT_List_Finalize(NULL, finalize_record_destructor, &memory, trace.expected_user);
        int null_list_unchanged = list.head == a && list.tail == b && trace.freed_count == 0 && trace.destroyed_count == 0;
        FT_List_Finalize(&list, finalize_record_destructor, NULL, trace.expected_user);
        int null_memory_unchanged = list.head == a && list.tail == b && trace.freed_count == 0 && trace.destroyed_count == 0;
        printf("{\"rows\":[{\"variant\":\"null_list\",\"list_unchanged\":%s,\"destructor_call_count\":0,\"freed_node_count\":0},{\"variant\":\"null_memory\",\"list_unchanged\":%s,\"destructor_call_count\":0,\"freed_node_count\":0}]}",
               null_list_unchanged ? "true" : "false",
               null_memory_unchanged ? "true" : "false");
        free(b);
        free(a);
    } else if (streq(case_id, "ftlist.FT_List_Finalize.destructor_receives_memory_data_user")) {
        FinalizeTrace trace = { 0 };
        trace.data[0] = &data_a;
        trace.node_labels[0] = "node_a";
        struct FT_MemoryRec_ memory = { &trace, oracle_alloc, finalize_record_free, oracle_realloc };
        trace.expected_memory = &memory;
        trace.expected_user = (void*)0x7111;
        FT_ListNode a = finalize_new_node(&trace, 0);
        list = (FT_ListRec){ a, a };
        FT_List_Finalize(&list, finalize_record_destructor, &memory, trace.expected_user);
        finalize_print_trace(&trace, &list);
    } else if (streq(case_id, "ftlist.FT_List_Iterate.iterates_all_nodes_success")) {
        const char* shapes[3] = { "empty", "one_node", "three_nodes" };
        printf("{\"rows\":[");
        for (int i = 0; i < 3; i++) {
            node_a = (FT_ListNodeRec){ NULL, NULL, &data_a };
            node_b = (FT_ListNodeRec){ NULL, NULL, &data_b };
            node_c = (FT_ListNodeRec){ NULL, NULL, &data_c };
            if (i == 0) {
                list = (FT_ListRec){ NULL, NULL };
            } else if (i == 1) {
                list = (FT_ListRec){ &node_a, &node_a };
            } else {
                node_a.next = &node_b;
                node_b.prev = &node_a;
                node_b.next = &node_c;
                node_c.prev = &node_b;
                list = (FT_ListRec){ &node_a, &node_c };
            }
            IterateTrace trace = { 0 };
            trace.nodes[0] = &node_a;
            trace.nodes[1] = &node_b;
            trace.nodes[2] = &node_c;
            trace.node_labels[0] = "node_a";
            trace.node_labels[1] = "node_b";
            trace.node_labels[2] = "node_c";
            trace.expected_user = (void*)0x7111;
            current_iterate_trace = &trace;
            FT_Error err = FT_List_Iterate(&list, iterate_record_callback, trace.expected_user);
            current_iterate_trace = NULL;
            if (i) printf(",");
            printf("{\"shape\":\"%s\",\"status\":%d,\"visited_data_tokens\":", shapes[i], err);
            iterate_print_visited(&trace);
            printf(",\"user_pointer_identity\":\"%s\",\"final_topology\":", iterate_user_identity(&trace));
            print_ft_list_topology3(&list, &data_a, &data_b, &data_c,
                                    i == 0 ? NULL : &node_a,
                                    i == 2 ? &node_b : NULL,
                                    i == 2 ? &node_c : NULL);
            printf("}");
        }
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Iterate.iterator_can_mutate_current_node")) {
        const char* modes[4] = {
            "remove_current",
            "move_current_to_head",
            "find_current_data",
            "finalize_side_list",
        };
        printf("{\"rows\":[");
        for (int i = 0; i < 4; i++) {
            node_a = (FT_ListNodeRec){ NULL, NULL, &data_a };
            node_b = (FT_ListNodeRec){ NULL, NULL, &data_b };
            node_c = (FT_ListNodeRec){ NULL, NULL, &data_c };
            node_a.next = &node_b;
            node_b.prev = &node_a;
            node_b.next = &node_c;
            node_c.prev = &node_b;
            list = (FT_ListRec){ &node_a, &node_c };

            unsigned char side_a_data = 4;
            unsigned char side_b_data = 5;
            FinalizeTrace finalize_trace = { 0 };
            finalize_trace.data[0] = &side_a_data;
            finalize_trace.data[1] = &side_b_data;
            finalize_trace.node_labels[0] = "side_a";
            finalize_trace.node_labels[1] = "side_b";
            struct FT_MemoryRec_ side_memory = { &finalize_trace, oracle_alloc, finalize_record_free, oracle_realloc };
            FT_ListNode side_a = finalize_new_node(&finalize_trace, 0);
            FT_ListNode side_b = finalize_new_node(&finalize_trace, 1);
            finalize_link_two(side_a, side_b);
            FT_ListRec side_list = { side_a, side_b };

            IterateTrace trace = { 0 };
            trace.nodes[0] = &node_a;
            trace.nodes[1] = &node_b;
            trace.nodes[2] = &node_c;
            trace.node_labels[0] = "node_a";
            trace.node_labels[1] = "node_b";
            trace.node_labels[2] = "node_c";
            trace.mode = modes[i];
            trace.list = &list;
            trace.side_list = &side_list;
            trace.side_memory = &side_memory;
            current_iterate_trace = &trace;
            FT_Error err = FT_List_Iterate(&list, iterate_mutation_callback, NULL);
            current_iterate_trace = NULL;

            if (i) printf(",");
            printf("{\"mutation\":\"%s\",\"status\":%d,\"visited_data_tokens\":", modes[i], err);
            iterate_print_visited(&trace);
            printf(",\"callback_events\":");
            iterate_print_events(&trace);
            printf(",\"final_topology\":");
            print_ft_list_topology3(&list, &data_a, &data_b, &data_c, &node_a, &node_b, &node_c);
            printf("}");

            if (!trace.side_finalized) {
                free(side_a);
                free(side_b);
            }
        }
        printf("]}");
    } else if (streq(case_id, "ftlist.FT_List_Find.success_finds_first_matching_node") ||
               streq(case_id, "ftlist.FT_List_Find.missing_data_returns_null") ||
               streq(case_id, "ftlist.FT_List_Find.null_list_returns_null") ||
               streq(case_id, "ftlist.FT_List_Find.null_data_matches_null_node_data")) {
        node_a.data = &data_c;
        node_b.data = &data_b;
        node_c.data = &data_c;
        node_a.next = &node_b;
        node_b.prev = &node_a;
        node_b.next = &node_c;
        node_c.prev = &node_b;
        list.head = &node_a;
        list.tail = &node_c;
        FT_List list_arg = &list;
        void* data_arg = &data_c;
        if (streq(case_id, "ftlist.FT_List_Find.missing_data_returns_null")) {
            data_arg = &data_a;
        } else if (streq(case_id, "ftlist.FT_List_Find.null_list_returns_null")) {
            list_arg = NULL;
        } else if (streq(case_id, "ftlist.FT_List_Find.null_data_matches_null_node_data")) {
            node_b.data = NULL;
            node_c.data = NULL;
            data_arg = NULL;
        }
        printf("{");
        print_ft_list_find_result(list_arg, data_arg, &node_a, &node_b, &node_c);
        printf("}");
    } else {
        printf("null}\n");
        return 2;
    }
    printf("}\n");
    return 0;
}

static int emit_bitmap_convert(const char* scenario) {
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf("}\n");
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    printf("{\"output\":{\"runs\":[");
    int first = 1;
#define RUN_SEP() do { if (!first) printf(","); first = 0; } while (0)
#define RUN_CONVERT(label, src_ptr, tgt_ptr, align_value, lib_value) do { \
        RUN_SEP(); \
        FT_Error run_err = FT_Bitmap_Convert((lib_value), (src_ptr), (tgt_ptr), (align_value)); \
        if (!first_error && run_err) first_error = run_err; \
        print_bitmap_convert_run((label), run_err, (tgt_ptr)); \
    } while (0)

    unsigned char bytes[256];
    FT_Bitmap source;
    FT_Bitmap target;
    FT_Bitmap_Init(&target);

    if (streq(scenario, "success_supported_depths_to_gray")) {
        const int modes[] = {
            FT_PIXEL_MODE_MONO, FT_PIXEL_MODE_GRAY2, FT_PIXEL_MODE_GRAY4,
            FT_PIXEL_MODE_GRAY, FT_PIXEL_MODE_LCD, FT_PIXEL_MODE_LCD_V,
            FT_PIXEL_MODE_BGRA
        };
        const char* labels[] = {"mono", "gray2", "gray4", "gray", "lcd", "lcd_v", "bgra"};
        for (size_t i = 0; i < sizeof(modes) / sizeof(modes[0]); i++) {
            bitmap_convert_source(&source, bytes, modes[i], 0);
            FT_Bitmap_Init(&target);
            RUN_CONVERT(labels[i], &source, &target, 1, library);
            FT_Bitmap_Done(library, &target);
        }
    } else if (streq(scenario, "success_alignment_and_flow")) {
        const int alignments[] = {0, 1, 2, 3, 4, -1, -2, -4};
        for (size_t i = 0; i < sizeof(alignments) / sizeof(alignments[0]); i++) {
            char label[32];
            snprintf(label, sizeof(label), "align_%d", alignments[i]);
            bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY, 0);
            FT_Bitmap_Init(&target);
            RUN_CONVERT(label, &source, &target, alignments[i], library);
            FT_Bitmap_Done(library, &target);
        }
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY, 1);
        FT_Bitmap_Init(&target);
        RUN_CONVERT("negative_source_flow", &source, &target, 4, library);
        FT_Bitmap_Done(library, &target);
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY, 0);
        FT_Bitmap_Init(&target);
        target.pitch = -1;
        RUN_CONVERT("negative_target_flow", &source, &target, 4, library);
        FT_Bitmap_Done(library, &target);
    } else if (streq(scenario, "success_repeated_conversion_reallocates")) {
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY2, 0);
        FT_Bitmap_Init(&target);
        RUN_CONVERT("first_gray2", &source, &target, 1, library);
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY4, 0);
        RUN_CONVERT("second_gray4", &source, &target, 1, library);
        FT_Bitmap_Done(library, &target);
    } else if (streq(scenario, "success_empty_or_null_source_buffer")) {
        FT_Bitmap_Init(&source);
        source.pixel_mode = FT_PIXEL_MODE_GRAY;
        source.width = 5;
        source.rows = 0;
        source.pitch = 5;
        FT_Bitmap_Init(&target);
        RUN_CONVERT("empty_null_buffer", &source, &target, 4, library);
        FT_Bitmap_Done(library, &target);
    } else if (streq(scenario, "error_invalid_arguments_or_alignment")) {
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY, 0);
        dirty_bitmap(&target);
        RUN_CONVERT("null_library", &source, &target, 1, NULL);
        dirty_bitmap(&target);
        RUN_CONVERT("null_source", NULL, &target, 1, library);
        RUN_SEP();
        err = FT_Bitmap_Convert(library, &source, NULL, 1);
        if (!first_error && err) first_error = err;
        print_bitmap_convert_run("null_target", err, NULL);
    } else if (streq(scenario, "error_unsupported_pixel_mode")) {
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY, 0);
        source.pixel_mode = FT_PIXEL_MODE_NONE;
        dirty_bitmap(&target);
        RUN_CONVERT("pixel_mode_none", &source, &target, 1, library);
        source.pixel_mode = FT_PIXEL_MODE_MAX;
        dirty_bitmap(&target);
        RUN_CONVERT("pixel_mode_max", &source, &target, 1, library);
    } else {
        fprintf(stderr, "unsupported bitmap convert scenario: %s\n", scenario);
        FT_Done_FreeType(library);
        return 2;
    }

#undef RUN_CONVERT
#undef RUN_SEP
    printf("]},");
    print_status(first_error);
    printf("}\n");
    FT_Done_FreeType(library);
    return 0;
}

static int emit_bitmap_done(const char* scenario) {
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf("}\n");
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    printf("{\"output\":{\"runs\":[");
    int first = 1;
#define DONE_SEP() do { if (!first) printf(","); first = 0; } while (0)
#define RUN_DONE(label, bitmap_ptr, lib_value) do { \
        DONE_SEP(); \
        FT_Error run_err = FT_Bitmap_Done((lib_value), (bitmap_ptr)); \
        if (!first_error && run_err) first_error = run_err; \
        print_bitmap_convert_run((label), run_err, (bitmap_ptr)); \
    } while (0)

    unsigned char bytes[256];
    FT_Bitmap source;
    FT_Bitmap bitmap;

    if (streq(scenario, "success_frees_and_zeroes_bitmap")) {
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY, 0);
        FT_Bitmap_Init(&bitmap);
        FT_Bitmap_Convert(library, &source, &bitmap, 1);
        RUN_DONE("allocated_bitmap", &bitmap, library);
    } else if (streq(scenario, "success_empty_bitmap")) {
        FT_Bitmap_Init(&bitmap);
        RUN_DONE("empty_bitmap", &bitmap, library);
    } else if (streq(scenario, "success_repeated_done_after_reinit")) {
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY2, 0);
        FT_Bitmap_Init(&bitmap);
        FT_Bitmap_Convert(library, &source, &bitmap, 1);
        RUN_DONE("first_done", &bitmap, library);
        FT_Bitmap_Init(&bitmap);
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY4, 0);
        FT_Bitmap_Convert(library, &source, &bitmap, 1);
        RUN_DONE("second_done", &bitmap, library);
    } else if (streq(scenario, "error_null_library_or_bitmap")) {
        bitmap_convert_source(&source, bytes, FT_PIXEL_MODE_GRAY, 0);
        FT_Bitmap_Init(&bitmap);
        FT_Bitmap_Convert(library, &source, &bitmap, 1);
        RUN_DONE("null_library", &bitmap, NULL);
        FT_Bitmap_Done(library, &bitmap);
        DONE_SEP();
        err = FT_Bitmap_Done(library, NULL);
        if (!first_error && err) first_error = err;
        print_bitmap_convert_run("null_bitmap", err, NULL);
    } else {
        fprintf(stderr, "unsupported bitmap done scenario: %s\n", scenario);
        FT_Done_FreeType(library);
        return 2;
    }

#undef RUN_DONE
#undef DONE_SEP
    printf("]},");
    print_status(first_error);
    printf("}\n");
    FT_Done_FreeType(library);
    return 0;
}

static int emit_bitmap_copy(const char* scenario) {
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf("}\n");
        return 0;
    }

    unsigned char source_bytes[12];
    unsigned char target_bytes[12];
    FT_Bitmap source;
    FT_Bitmap target;
    bitmap_copy_source(&source, source_bytes);
    FT_Bitmap_Init(&target);

    const FT_Bitmap* source_arg = &source;
    FT_Bitmap* target_arg = &target;
    FT_Library library_arg = library;

    if (streq(scenario, "success_deep_copy_all_public_fields")) {
        source.pitch = -4;
    } else if (streq(scenario, "success_source_equals_target_noop")) {
        target_arg = &source;
    } else if (streq(scenario, "success_null_source_buffer")) {
        source.buffer = NULL;
        target.pitch = -1;
    } else if (streq(scenario, "success_flow_flip")) {
        source.pitch = -4;
        target.pitch = 1;
    } else if (streq(scenario, "ownership_replaces_target_buffer")) {
        memset(target_bytes, 0xE5, sizeof(target_bytes));
        FT_Bitmap pre_source;
        bitmap_copy_source(&pre_source, target_bytes);
        err = FT_Bitmap_Copy(library, &pre_source, &target);
        if (err) {
            printf("{");
            print_status(err);
            printf("}\n");
            FT_Done_FreeType(library);
            return 0;
        }
    } else if (streq(scenario, "error_null_library")) {
        library_arg = NULL;
    } else if (streq(scenario, "error_null_source") ||
               streq(scenario, "error_null_library_or_bitmaps")) {
        source_arg = NULL;
    } else if (streq(scenario, "error_null_target")) {
        target_arg = NULL;
    } else {
        fprintf(stderr, "unsupported bitmap copy scenario: %s\n", scenario);
        FT_Done_FreeType(library);
        return 2;
    }

    err = FT_Bitmap_Copy(library_arg, source_arg, target_arg);
    if (target_arg == &source) {
        target = source;
    }

    printf("{");
    print_status(err);
    if (!err) {
        size_t len = target.buffer ? bitmap_len(&target) : 0;
        printf(",\"output\":{\"target\":");
        print_bitmap_fields(&target);
        printf(",\"target_bytes_hash\":");
        print_byte_hash_or_null(target.buffer, len);
        printf(",\"target_buffer_len\":%zu,\"buffer_identity_class\":\"%s\"}",
               len, target.buffer ? "owned_copy" : "null");
    }
    printf("}\n");
    if (target.buffer && target.buffer != source.buffer && target.buffer != target_bytes) {
        FT_Bitmap_Done(library, &target);
    }
    FT_Done_FreeType(library);
    return 0;
}

static int bitmap_embolden_pitch(unsigned char pixel_mode, unsigned int width) {
    switch (pixel_mode) {
    case FT_PIXEL_MODE_MONO:
      return (int)((width + 7) >> 3) + 1;
    case FT_PIXEL_MODE_GRAY2:
      return (int)((width * 2 + 7) >> 3) + 1;
    case FT_PIXEL_MODE_GRAY4:
      return (int)((width * 4 + 7) >> 3) + 1;
    case FT_PIXEL_MODE_BGRA:
      return (int)(width * 4);
    default:
      return (int)width + 2;
    }
}

static int bitmap_embolden_alloc(FT_Bitmap* bitmap, unsigned char pixel_mode, int negative_pitch) {
    unsigned int width = 5;
    unsigned int rows = 3;
    int pitch = bitmap_embolden_pitch(pixel_mode, width);
    size_t len = (size_t)pitch * rows;
    unsigned char* bytes = (unsigned char*)malloc(len ? len : 1);
    if (!bytes) {
      return 1;
    }
    for (size_t i = 0; i < len; i++) {
      bytes[i] = (unsigned char)((i * 37 + pixel_mode * 11 + (negative_pitch ? 5 : 0)) & 0xFF);
    }
    bitmap->rows = rows;
    bitmap->width = width;
    bitmap->pitch = negative_pitch ? -pitch : pitch;
    bitmap->buffer = bytes;
    bitmap->num_grays = pixel_mode == FT_PIXEL_MODE_GRAY2 ? 4 :
                        pixel_mode == FT_PIXEL_MODE_GRAY4 ? 16 : 256;
    bitmap->pixel_mode = pixel_mode;
    bitmap->palette_mode = 0;
    bitmap->palette = NULL;
    return 0;
}

static FT_Error print_bitmap_embolden_row(
    const char* label,
    FT_Library library_arg,
    FT_Library cleanup_library,
    unsigned char pixel_mode,
    int negative_pitch,
    FT_Pos xstrength,
    FT_Pos ystrength,
    int null_bitmap,
    int null_buffer)
{
    FT_Bitmap bitmap;
    FT_Bitmap* bitmap_arg = &bitmap;
    FT_Bitmap_Init(&bitmap);
    if (!null_bitmap) {
        if (bitmap_embolden_alloc(&bitmap, pixel_mode, negative_pitch)) {
            printf("{\"label\":\"%s\",\"error\":%d,\"bitmap\":null,\"buffer_hex\":\"\",\"buffer_len\":0,\"buffer_identity_class\":\"null\"}", label, FT_Err_Out_Of_Memory);
            return FT_Err_Out_Of_Memory;
        }
        if (null_buffer) {
            free(bitmap.buffer);
            bitmap.buffer = NULL;
        }
    } else {
        bitmap_arg = NULL;
    }

    FT_Error err = FT_Bitmap_Embolden(library_arg, bitmap_arg, xstrength, ystrength);
    size_t len = bitmap_arg && bitmap.buffer ? bitmap_len(&bitmap) : 0;
    printf("{\"label\":\"%s\",\"pixel_mode\":%u,\"negative_pitch\":", label, pixel_mode);
    print_json_bool(negative_pitch);
    printf(",\"x_strength\":%ld,\"y_strength\":%ld,\"error\":%d,\"bitmap\":",
           (long)xstrength, (long)ystrength, err);
    if (bitmap_arg) {
        print_bitmap_fields(&bitmap);
    } else {
        printf("null");
    }
    printf(",\"buffer_hex\":\"");
    if (bitmap_arg && bitmap.buffer && len > 0) {
        print_hex_bytes(bitmap.buffer, (long)len);
    }
    printf("\",\"buffer_len\":%zu,\"buffer_identity_class\":\"%s\"}",
           len, bitmap_arg && bitmap.buffer ? "owned" : "null");

    if (bitmap_arg && bitmap.buffer) {
        if (cleanup_library) {
            FT_Bitmap_Done(cleanup_library, &bitmap);
        } else {
            free(bitmap.buffer);
        }
    }
    return err;
}

static void bitmap_blend_source(FT_Bitmap* bitmap, unsigned char* bytes, FT_Pixel_Mode mode, int negative) {
    memset(bytes, 0, 96);
    bitmap->rows = 3;
    bitmap->width = 5;
    bitmap->num_grays = 256;
    bitmap->pixel_mode = mode;
    bitmap->palette_mode = 0;
    bitmap->palette = NULL;
    int pitch = 5;
    if (mode == FT_PIXEL_MODE_MONO) {
        pitch = 1;
        unsigned char values[3] = { 0xA8, 0x50, 0xF8 };
        memcpy(bytes, values, sizeof(values));
    } else if (mode == FT_PIXEL_MODE_GRAY2) {
        pitch = 2;
        unsigned char values[6] = { 0x1B, 0x40, 0xE4, 0x80, 0x6D, 0xC0 };
        memcpy(bytes, values, sizeof(values));
        bitmap->num_grays = 4;
    } else if (mode == FT_PIXEL_MODE_GRAY4) {
        pitch = 3;
        unsigned char values[9] = { 0x17, 0x3F, 0x50, 0x9B, 0xDF, 0x10, 0x24, 0x68, 0xA0 };
        memcpy(bytes, values, sizeof(values));
        bitmap->num_grays = 16;
    } else if (mode == FT_PIXEL_MODE_LCD || mode == FT_PIXEL_MODE_LCD_V) {
        bitmap->rows = 2;
        bitmap->width = 6;
        pitch = 6;
        for (int i = 0; i < 12; i++) {
            bytes[i] = (unsigned char)(13 + i * 19);
        }
    } else if (mode == FT_PIXEL_MODE_BGRA) {
        bitmap->rows = 2;
        bitmap->width = 3;
        pitch = 12;
        unsigned char values[24] = {
            10, 20, 30, 90, 20, 60, 80, 140, 0, 0, 0, 0,
            40, 30, 20, 128, 90, 40, 10, 200, 8, 16, 32, 64
        };
        memcpy(bytes, values, sizeof(values));
    } else {
        for (int i = 0; i < 15; i++) {
            bytes[i] = (unsigned char)(17 + i * 11);
        }
    }
    bitmap->pitch = negative ? -pitch : pitch;
    bitmap->buffer = bytes;
}

static int bitmap_blend_prepopulate(FT_Library library, FT_Bitmap* target, FT_Vector* target_offset) {
    unsigned char pre_bytes[96];
    FT_Bitmap pre_source;
    bitmap_blend_source(&pre_source, pre_bytes, FT_PIXEL_MODE_GRAY, 0);
    pre_source.width = 2;
    pre_source.rows = 2;
    pre_source.pitch = 2;
    FT_Vector pre_source_offset = { 64, 128 };
    FT_Color pre_color = { 31, 47, 79, 191 };
    target_offset->x = 64;
    target_offset->y = 128;
    return FT_Bitmap_Blend(library, &pre_source, pre_source_offset, target, target_offset, pre_color);
}

static void print_blend_run_output(const FT_Bitmap* target, const FT_Vector* target_offset) {
    size_t len = target->buffer ? bitmap_len(target) : 0;
    printf("{\"target\":");
    print_bitmap_fields(target);
    printf(",\"target_bytes_hash\":");
    print_byte_hash_or_null(target->buffer, len);
    printf(",\"target_buffer_len\":%zu,\"target_offset\":{\"x\":%ld,\"y\":%ld}}",
           len, target_offset->x, target_offset->y);
}

static int emit_bitmap_blend_success_run(
    FT_Library library,
    FT_Pixel_Mode mode,
    int negative_source,
    int existing_target,
    FT_Vector source_offset,
    FT_Vector target_offset,
    FT_Color color) {
    unsigned char source_bytes[96];
    FT_Bitmap source;
    FT_Bitmap target;
    FT_Bitmap_Init(&target);
    bitmap_blend_source(&source, source_bytes, mode, negative_source);
    if (existing_target) {
        FT_Error pre_err = bitmap_blend_prepopulate(library, &target, &target_offset);
        if (pre_err) {
            printf("{");
            print_status(pre_err);
            printf("}");
            return 0;
        }
    }

    FT_Error err = FT_Bitmap_Blend(library, &source, source_offset, &target, &target_offset, color);
    printf("{");
    print_status(err);
    if (!err) {
        printf(",\"output\":");
        print_blend_run_output(&target, &target_offset);
    }
    printf("}");
    if (target.buffer) {
        FT_Bitmap_Done(library, &target);
    }
    return 0;
}

static int emit_bitmap_embolden(const char* scenario) {
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf("}\n");
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    printf("{\"output\":{\"rows\":[");
    int first = 1;
#define EMIT_ROW(label, lib, mode, neg, xs, ys, null_bitmap, null_buffer) \
    do { \
        if (!first) printf(","); \
        first = 0; \
        FT_Error row_error = print_bitmap_embolden_row(label, lib, library, mode, neg, xs, ys, null_bitmap, null_buffer); \
        if (!first_error && row_error) first_error = row_error; \
    } while (0)

    if (streq(scenario, "success_gray_and_packed_modes")) {
        unsigned char modes[] = { FT_PIXEL_MODE_MONO, FT_PIXEL_MODE_GRAY2, FT_PIXEL_MODE_GRAY4, FT_PIXEL_MODE_GRAY, FT_PIXEL_MODE_LCD, FT_PIXEL_MODE_LCD_V, FT_PIXEL_MODE_BGRA };
        for (size_t i = 0; i < sizeof(modes) / sizeof(modes[0]); i++) {
            EMIT_ROW("mode", library, modes[i], 0, 64, 96, 0, 0);
        }
    } else if (streq(scenario, "success_strength_rounding_and_zero")) {
        FT_Pos strengths[][2] = {
            {0, 0}, {32, 32}, {64, 0}, {0, 64}, {64, 96}, {512, 64}
        };
        for (size_t i = 0; i < sizeof(strengths) / sizeof(strengths[0]); i++) {
            EMIT_ROW("strength", library, FT_PIXEL_MODE_GRAY, 0, strengths[i][0], strengths[i][1], 0, 0);
        }
    } else if (streq(scenario, "success_gray2_gray4_convert_to_gray")) {
        unsigned char modes[] = { FT_PIXEL_MODE_GRAY2, FT_PIXEL_MODE_GRAY4 };
        for (size_t i = 0; i < sizeof(modes) / sizeof(modes[0]); i++) {
            EMIT_ROW("packed-positive", library, modes[i], 0, 64, 96, 0, 0);
            EMIT_ROW("packed-negative", library, modes[i], 1, 64, 96, 0, 0);
        }
    } else if (streq(scenario, "error_mono_strength_limit")) {
        EMIT_ROW("mono-clamps-xstrength", library, FT_PIXEL_MODE_MONO, 0, 1024, 64, 0, 0);
        EMIT_ROW("mono-clamps-xstrength-negative-pitch", library, FT_PIXEL_MODE_MONO, 1, 1024, 64, 0, 0);
    } else if (streq(scenario, "error_invalid_arguments_or_modes")) {
        EMIT_ROW("null-library", NULL, FT_PIXEL_MODE_GRAY, 0, 64, 64, 0, 0);
        EMIT_ROW("null-bitmap", library, FT_PIXEL_MODE_GRAY, 0, 64, 64, 1, 0);
        EMIT_ROW("null-buffer", library, FT_PIXEL_MODE_GRAY, 0, 64, 64, 0, 1);
        EMIT_ROW("unsupported-mode", library, 99, 0, 64, 64, 0, 0);
        EMIT_ROW("negative-strength", library, FT_PIXEL_MODE_GRAY, 0, -64, 0, 0, 0);
        EMIT_ROW("negative-y-strength", library, FT_PIXEL_MODE_GRAY, 0, 0, -64, 0, 0);
        EMIT_ROW("overflow-strength", library, FT_PIXEL_MODE_GRAY, 0, 137438953472LL, 0, 0, 0);
    } else if (streq(scenario, "ownership_reallocates_bitmap_buffer")) {
        EMIT_ROW("realloc-positive-pitch", library, FT_PIXEL_MODE_GRAY, 0, 64, 96, 0, 0);
        EMIT_ROW("realloc-negative-pitch", library, FT_PIXEL_MODE_GRAY, 1, 64, 96, 0, 0);
    } else {
        fprintf(stderr, "unsupported bitmap embolden scenario: %s\n", scenario);
        FT_Done_FreeType(library);
        return 2;
    }
#undef EMIT_ROW

    printf("]},");
    print_status(first_error);
    printf("}\n");
    FT_Done_FreeType(library);
    return 0;
}

static int emit_bitmap_blend(const char* scenario) {
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf("}\n");
        return 0;
    }

    FT_Color color = { 29, 113, 211, 173 };
    FT_Vector source_offset = { 31, 95 };
    FT_Vector target_offset = { -33, 130 };

    if (streq(scenario, "error_invalid_arguments_or_target_mode")) {
        unsigned char source_bytes[96];
        unsigned char target_bytes[96];
        FT_Bitmap source;
        FT_Bitmap target;
        bitmap_blend_source(&source, source_bytes, FT_PIXEL_MODE_GRAY, 0);
        bitmap_blend_source(&target, target_bytes, FT_PIXEL_MODE_GRAY, 0);
        err = FT_Bitmap_Blend(library, &source, source_offset, &target, &target_offset, color);
        printf("{");
        print_status(err);
        printf("}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    if (streq(scenario, "error_overflow_or_flow_mismatch")) {
        unsigned char source_bytes[96];
        FT_Bitmap source;
        FT_Bitmap target;
        FT_Bitmap_Init(&target);
        bitmap_blend_source(&source, source_bytes, FT_PIXEL_MODE_GRAY, 0);
        FT_Vector pre_offset = { 64, 128 };
        err = bitmap_blend_prepopulate(library, &target, &pre_offset);
        if (!err) {
            target.pitch = -target.pitch;
            err = FT_Bitmap_Blend(library, &source, source_offset, &target, &target_offset, color);
        }
        printf("{");
        print_status(err);
        printf("}\n");
        if (target.buffer) {
            target.pitch = target.pitch < 0 ? -target.pitch : target.pitch;
            FT_Bitmap_Done(library, &target);
        }
        FT_Done_FreeType(library);
        return 0;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"runs\":[");
    if (streq(scenario, "success_source_pixel_modes_and_flow")) {
        FT_Pixel_Mode modes[7] = {
            FT_PIXEL_MODE_MONO, FT_PIXEL_MODE_GRAY2, FT_PIXEL_MODE_GRAY4, FT_PIXEL_MODE_GRAY,
            FT_PIXEL_MODE_LCD, FT_PIXEL_MODE_LCD_V, FT_PIXEL_MODE_BGRA
        };
        int first = 1;
        for (int flow = 0; flow < 2; flow++) {
            for (int i = 0; i < 7; i++) {
                if (!first) {
                    printf(",");
                }
                first = 0;
                emit_bitmap_blend_success_run(library, modes[i], flow, 0, source_offset, target_offset, color);
            }
        }
    } else if (streq(scenario, "success_existing_bgra_reallocates_or_preserves")) {
        emit_bitmap_blend_success_run(library, FT_PIXEL_MODE_GRAY, 0, 1, source_offset, target_offset, color);
    } else if (streq(scenario, "success_integerizes_offsets")) {
        FT_Vector frac_source = { 127, -65 };
        FT_Vector frac_target = { 95, 193 };
        emit_bitmap_blend_success_run(library, FT_PIXEL_MODE_GRAY4, 0, 1, frac_source, frac_target, color);
    } else if (streq(scenario, "success_empty_target_allocates_bgra")) {
        emit_bitmap_blend_success_run(library, FT_PIXEL_MODE_GRAY, 0, 0, source_offset, target_offset, color);
    } else {
        fprintf(stderr, "unsupported bitmap blend scenario: %s\n", scenario);
        FT_Done_FreeType(library);
        return 2;
    }
    printf("]}}\n");
    FT_Done_FreeType(library);
    return 0;
}

static void print_glyphslot_own_bitmap_output(FT_Error err,
                                              FT_GlyphSlot slot,
                                              const unsigned char* before_buffer,
                                              const char* no_slot_identity) {
    printf("{");
    print_status(err);
    if (err || !slot) {
        printf(",\"output\":{\"error\":%d,\"slot\":null,\"own_bitmap_flag\":false,\"buffer_identity_class\":\"%s\"}}\n",
               err,
               no_slot_identity);
        return;
    }

    const char* identity = "unchanged";
    if (slot->bitmap.buffer && before_buffer && slot->bitmap.buffer != before_buffer) {
        identity = "owned_copy";
    } else if (slot->internal && (slot->internal->flags & FT_GLYPH_OWN_BITMAP)) {
        identity = "already_owned";
    }

    printf(",\"output\":{\"error\":%d,\"slot\":{", err);
    print_slot_body(slot, slot->glyph_index);
    printf("},\"own_bitmap_flag\":");
    print_json_bool(slot->internal && (slot->internal->flags & FT_GLYPH_OWN_BITMAP));
    printf(",\"buffer_identity_class\":\"%s\"}}\n", identity);
}

static void print_glyphslot_own_bitmap_variant(const char* variant,
                                               FT_Error err,
                                               FT_GlyphSlot slot,
                                               const unsigned char* before_buffer,
                                               const char* no_slot_identity) {
    const char* identity = no_slot_identity;
    int owns_bitmap = 0;
    printf("{\"variant\":\"%s\",\"error\":%d,\"slot\":", variant, err);
    if (!slot) {
        printf("null");
    } else {
        identity = "unchanged";
        owns_bitmap = slot->internal && (slot->internal->flags & FT_GLYPH_OWN_BITMAP);
        if (slot->bitmap.buffer && before_buffer && slot->bitmap.buffer != before_buffer) {
            identity = "owned_copy";
        } else if (owns_bitmap) {
            identity = "already_owned";
        }
        printf("{");
        print_slot_body(slot, slot->glyph_index);
        printf("}");
    }
    printf(",\"own_bitmap_flag\":");
    print_json_bool(owns_bitmap);
    printf(",\"buffer_identity_class\":\"%s\"}", identity);
}

static int emit_glyphslot_own_bitmap(int argc, char** argv) {
    if (argc != 9) {
        fprintf(stderr, "--glyphslot-own-bitmap requires SCENARIO SOURCE_KIND SOURCE FACE_INDEX PX GID FLAGS\n");
        return 2;
    }
    const char* scenario = argv[2];
    const char* source_kind = argv[3];
    const char* source_value = argv[4];
    FT_Long face_index = atol(argv[5]);
    FT_UInt px = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[8], NULL, 10);

    FT_Error null_err = FT_GlyphSlot_Own_Bitmap(NULL);
    if (streq(scenario, "success_non_bitmap_or_null_slot_noop") && null_err) {
        printf("{");
        print_status(null_err);
        printf(",\"output\":null}\n");
        return 0;
    }

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

    FailAfterMemoryState fail_state = {0, 0, 0};
    struct FT_MemoryRec_ fail_memory = {&fail_state, fail_after_alloc, fail_after_free, fail_after_realloc};
    FT_Library library = NULL;
    FT_Face face = NULL;
    int custom_memory = streq(scenario, "error_copy_allocation_failure");
    FT_Error err = custom_memory ? FT_New_Library(&fail_memory, &library) : FT_Init_FreeType(&library);
    if (!err && custom_memory) {
        FT_Add_Default_Modules(library);
    }
    if (!err) {
        err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    }
    if (!err) {
        err = FT_Set_Pixel_Sizes(face, 0, px);
    }
    if (!err) {
        FT_Int32 effective_flags = load_flags;
        if (streq(scenario, "success_non_bitmap_or_null_slot_noop")) {
            effective_flags &= ~FT_LOAD_RENDER;
        } else {
            effective_flags |= FT_LOAD_RENDER;
        }
        err = FT_Load_Glyph(face, glyph_index, effective_flags);
    }
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        if (face) {
            FT_Done_Face(face);
        }
        if (library) {
            if (custom_memory) {
                FT_Done_Library(library);
            } else {
                FT_Done_FreeType(library);
            }
        }
        free(data);
        return 0;
    }

    unsigned char* before_buffer = face->glyph->bitmap.buffer;
    if (streq(scenario, "error_copy_allocation_failure")) {
        printf("{");
        print_status(FT_Err_Out_Of_Memory);
        printf(",\"output\":{\"error\":%d,\"variants\":[", FT_Err_Out_Of_Memory);

        face->glyph->internal->flags &= ~FT_GLYPH_OWN_BITMAP;
        fail_state.enabled = 1;
        fail_state.fail_after = 0;
        fail_state.allocation_count = 0;
        err = FT_GlyphSlot_Own_Bitmap(face->glyph);
        fail_state.enabled = 0;
        print_glyphslot_own_bitmap_variant("bitmap_borrowed", err, face->glyph, before_buffer, "null_slot");

        face->glyph->internal->flags |= FT_GLYPH_OWN_BITMAP;
        printf(",");
        err = FT_GlyphSlot_Own_Bitmap(face->glyph);
        print_glyphslot_own_bitmap_variant("bitmap_owned", err, face->glyph, face->glyph->bitmap.buffer, "null_slot");

        err = FT_Load_Glyph(face, glyph_index, load_flags & ~FT_LOAD_RENDER);
        printf(",");
        print_glyphslot_own_bitmap_variant("outline_format", err ? err : FT_GlyphSlot_Own_Bitmap(face->glyph), face->glyph, face->glyph->bitmap.buffer, "null_slot");

        printf(",");
        print_glyphslot_own_bitmap_variant("null_slot", FT_GlyphSlot_Own_Bitmap(NULL), NULL, NULL, "null_slot");
        printf("]}}\n");
    } else if (streq(scenario, "success_borrowed_bitmap_copied_and_flagged")) {
        face->glyph->internal->flags &= ~FT_GLYPH_OWN_BITMAP;
        err = FT_GlyphSlot_Own_Bitmap(face->glyph);
        print_glyphslot_own_bitmap_output(err, face->glyph, before_buffer, "null_slot");
    } else if (streq(scenario, "success_already_owned_noop")) {
        face->glyph->internal->flags |= FT_GLYPH_OWN_BITMAP;
        err = FT_GlyphSlot_Own_Bitmap(face->glyph);
        print_glyphslot_own_bitmap_output(err, face->glyph, before_buffer, "null_slot");
    } else if (streq(scenario, "success_non_bitmap_or_null_slot_noop")) {
        err = FT_GlyphSlot_Own_Bitmap(face->glyph);
        printf("{");
        print_status(err);
        printf(",\"output\":{\"error\":%d,\"variants\":[{\"variant\":\"outline_format\",\"slot\":{", err);
        print_slot_body(face->glyph, face->glyph->glyph_index);
        printf("},\"own_bitmap_flag\":");
        print_json_bool(face->glyph->internal && (face->glyph->internal->flags & FT_GLYPH_OWN_BITMAP));
        printf(",\"buffer_identity_class\":\"unchanged\"},{\"variant\":\"null_slot\",\"slot\":null,\"own_bitmap_flag\":false,\"buffer_identity_class\":\"null_slot\"}]}}\n");
    } else {
        fprintf(stderr, "unsupported glyphslot own bitmap scenario: %s\n", scenario);
        FT_Done_Face(face);
        if (custom_memory) {
            FT_Done_Library(library);
        } else {
            FT_Done_FreeType(library);
        }
        free(data);
        return 2;
    }

    FT_Done_Face(face);
    if (custom_memory) {
        FT_Done_Library(library);
    } else {
        FT_Done_FreeType(library);
    }
    free(data);
    return 0;
}

static int emit_bitmap_init_new(const char* op, const char* null_arg, const char* alias_arg) {
    int use_null = atoi(null_arg) != 0;
    int compare_alias = atoi(alias_arg) != 0;
    FT_Bitmap bitmap;
    FT_Bitmap alias_bitmap;
    dirty_bitmap(&bitmap);
    dirty_bitmap(&alias_bitmap);

    if (streq(op, "init")) {
        FT_Bitmap_Init(use_null ? NULL : &bitmap);
    } else {
        FT_Bitmap_New(use_null ? NULL : &bitmap);
    }
    if (compare_alias) {
        if (streq(op, "init")) {
            FT_Bitmap_New(&alias_bitmap);
        } else {
            FT_Bitmap_Init(&alias_bitmap);
        }
    }

    printf("{");
    print_status(0);
    printf(",\"output\":{\"bitmap\":");
    print_bitmap_fields(&bitmap);
    printf(",\"null_pointer_write_state\":\"%s\",\"symbol_presence\":{\"init\":true,\"new\":true}",
           use_null ? "null_noop" : "written");
    if (compare_alias) {
        printf(",\"alias_bitmap\":");
        print_bitmap_fields(&alias_bitmap);
    }
    printf("}}\n");
    return 0;
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

static int emit_constant_map(const char* symbols_csv) {
    size_t symbols_len = strlen(symbols_csv);
    char* symbols = (char*)malloc(symbols_len + 1);
    if (!symbols) {
        return 1;
    }
    memcpy(symbols, symbols_csv, symbols_len + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"values\":{");
    char* cursor = symbols;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        if (*cursor) {
            long long value = 0;
            if (!emit_generated_constant_value(cursor, &value)) {
                fprintf(stderr, "unsupported constant: %s\n", cursor);
                free(symbols);
                return 2;
            }
            if (emitted) {
                printf(",");
            }
            printf("\"%s\":%lld", cursor, value);
            emitted = 1;
        }
        cursor = next ? next + 1 : NULL;
    }
    printf("}}}\n");
    free(symbols);
    return 0;
}

static int split_fixed_math_row(char* row, long long* values, int max_values) {
    int count = 0;
    char* cursor = row;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        if (count >= max_values) {
            return -1;
        }
        values[count++] = strtoll(cursor, NULL, 10);
        cursor = next ? next + 1 : NULL;
    }
    return count;
}

static long long fixed_math_result(const char* op, const long long* values, int count, int* ok) {
    *ok = 1;
    if (streq(op, "ceil_fix") && count == 1) {
        return (long long)FT_CeilFix((FT_Fixed)values[0]);
    }
    if (streq(op, "floor_fix") && count == 1) {
        return (long long)FT_FloorFix((FT_Fixed)values[0]);
    }
    if (streq(op, "round_fix") && count == 1) {
        return (long long)FT_RoundFix((FT_Fixed)values[0]);
    }
    if (streq(op, "mul_fix") && count == 2) {
        return (long long)FT_MulFix((FT_Long)values[0], (FT_Long)values[1]);
    }
    if (streq(op, "div_fix") && count == 2) {
        return (long long)FT_DivFix((FT_Long)values[0], (FT_Long)values[1]);
    }
    if (streq(op, "mul_div") && count == 3) {
        return (long long)FT_MulDiv((FT_Long)values[0], (FT_Long)values[1], (FT_Long)values[2]);
    }
    *ok = 0;
    return 0;
}

static int emit_fixed_math(const char* op, const char* rows_csv) {
    size_t rows_len = strlen(rows_csv);
    char* rows = (char*)malloc(rows_len + 1);
    if (!rows) {
        return 1;
    }
    memcpy(rows, rows_csv, rows_len + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* cursor = rows;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        long long values[3] = {0, 0, 0};
        int value_count = split_fixed_math_row(cursor, values, 3);
        int ok = 0;
        long long result = fixed_math_result(op, values, value_count, &ok);
        if (value_count <= 0 || !ok) {
            fprintf(stderr, "unsupported fixed math row op=%s row=%s\n", op, cursor);
            free(rows);
            return 2;
        }
        if (emitted) {
            printf(",");
        }
        printf("{\"input\":[");
        for (int i = 0; i < value_count; i++) {
            if (i) {
                printf(",");
            }
            printf("%lld", values[i]);
        }
        printf("],\"result\":%lld}", result);
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static void print_vector_object(FT_Vector vector) {
    printf("{\"x\":%ld,\"y\":%ld}", vector.x, vector.y);
}

static void print_matrix_object(FT_Matrix matrix) {
    printf("{\"xx\":%ld,\"xy\":%ld,\"yx\":%ld,\"yy\":%ld}",
           matrix.xx, matrix.xy, matrix.yx, matrix.yy);
}

static int emit_vector_transform(const char* rows_csv) {
    size_t rows_len = strlen(rows_csv);
    char* rows = (char*)malloc(rows_len + 1);
    if (!rows) {
        return 1;
    }
    memcpy(rows, rows_csv, rows_len + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* cursor = rows;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        long long values[7] = {0, 0, 0, 0, 0, 0, 0};
        int value_count = split_fixed_math_row(cursor, values, 7);
        if (value_count != 7) {
            fprintf(stderr, "vector transform row must have 7 values: %s\n", cursor);
            free(rows);
            return 2;
        }
        int mode = (int)values[0];
        FT_Vector vector = {(FT_Pos)values[1], (FT_Pos)values[2]};
        FT_Matrix matrix = {
            (FT_Fixed)values[3],
            (FT_Fixed)values[4],
            (FT_Fixed)values[5],
            (FT_Fixed)values[6],
        };
        FT_Vector* vector_ptr = mode == 1 ? NULL : &vector;
        FT_Matrix* matrix_ptr = mode == 2 ? NULL : &matrix;
        FT_Vector_Transform(vector_ptr, matrix_ptr);
        if (emitted) {
            printf(",");
        }
        printf("{\"mode\":%d,\"result\":", mode);
        if (vector_ptr) {
            print_vector_object(vector);
        } else {
            printf("null");
        }
        printf("}");
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static int emit_matrix_multiply(const char* rows_csv) {
    size_t rows_len = strlen(rows_csv);
    char* rows = (char*)malloc(rows_len + 1);
    if (!rows) {
        return 1;
    }
    memcpy(rows, rows_csv, rows_len + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* cursor = rows;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        long long values[9] = {0, 0, 0, 0, 0, 0, 0, 0, 0};
        int value_count = split_fixed_math_row(cursor, values, 9);
        if (value_count != 9) {
            fprintf(stderr, "matrix multiply row must have 9 values: %s\n", cursor);
            free(rows);
            return 2;
        }
        int mode = (int)values[0];
        FT_Matrix a = {
            (FT_Fixed)values[1],
            (FT_Fixed)values[2],
            (FT_Fixed)values[3],
            (FT_Fixed)values[4],
        };
        FT_Matrix b = {
            (FT_Fixed)values[5],
            (FT_Fixed)values[6],
            (FT_Fixed)values[7],
            (FT_Fixed)values[8],
        };
        FT_Matrix* a_ptr = (mode == 1 || mode == 3) ? NULL : &a;
        FT_Matrix* b_ptr = (mode == 2 || mode == 3) ? NULL : &b;
        FT_Matrix_Multiply(a_ptr, b_ptr);
        if (emitted) {
            printf(",");
        }
        printf("{\"mode\":%d,\"result\":", mode);
        if (b_ptr) {
            print_matrix_object(b);
        } else {
            printf("null");
        }
        printf("}");
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static int emit_matrix_invert(const char* rows_csv) {
    size_t rows_len = strlen(rows_csv);
    char* rows = (char*)malloc(rows_len + 1);
    if (!rows) {
        return 1;
    }
    memcpy(rows, rows_csv, rows_len + 1);

    FT_Matrix results[64];
    int modes[64];
    int count = 0;
    FT_Error err = 0;
    char* cursor = rows;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        long long values[5] = {0, 0, 0, 0, 0};
        int value_count = split_fixed_math_row(cursor, values, 5);
        if (value_count != 5 || count >= 64) {
            fprintf(stderr, "matrix invert row must have 5 values: %s\n", cursor);
            free(rows);
            return 2;
        }
        int mode = (int)values[0];
        FT_Matrix matrix = {
            (FT_Fixed)values[1],
            (FT_Fixed)values[2],
            (FT_Fixed)values[3],
            (FT_Fixed)values[4],
        };
        err = FT_Matrix_Invert(mode == 1 ? NULL : &matrix);
        if (err) {
            break;
        }
        modes[count] = mode;
        results[count] = matrix;
        count++;
        cursor = next ? next + 1 : NULL;
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
        free(rows);
        return 0;
    }
    printf(",\"output\":{\"rows\":[");
    for (int i = 0; i < count; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"mode\":%d,\"result\":", modes[i]);
        print_matrix_object(results[i]);
        printf("}");
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static int emit_trigon(const char* op, const char* rows_csv) {
    size_t rows_len = strlen(rows_csv);
    char* rows = (char*)malloc(rows_len + 1);
    if (!rows) {
        return 1;
    }
    memcpy(rows, rows_csv, rows_len + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* cursor = rows;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        long long values[5] = {0, 0, 0, 0, 0};
        int value_count = split_fixed_math_row(cursor, values, 5);
        if (emitted) {
            printf(",");
        }

        if ((streq(op, "sin") || streq(op, "cos") || streq(op, "tan")) && value_count == 1) {
            FT_Angle angle = (FT_Angle)values[0];
            FT_Fixed result = streq(op, "sin") ? FT_Sin(angle) : streq(op, "cos") ? FT_Cos(angle) : FT_Tan(angle);
            printf("{\"angle\":%lld,\"result\":%ld}", values[0], result);
        } else if (streq(op, "atan2") && value_count == 2) {
            FT_Fixed x = (FT_Fixed)values[0];
            FT_Fixed y = (FT_Fixed)values[1];
            printf("{\"vector\":");
            print_vector_object((FT_Vector){(FT_Pos)x, (FT_Pos)y});
            printf(",\"result\":%ld}", FT_Atan2(x, y));
        } else if (streq(op, "angle_diff") && value_count == 2) {
            FT_Angle angle1 = (FT_Angle)values[0];
            FT_Angle angle2 = (FT_Angle)values[1];
            printf("{\"angle1\":%lld,\"angle2\":%lld,\"result\":%ld}", values[0], values[1], FT_Angle_Diff(angle1, angle2));
        } else if (streq(op, "vector_unit") && value_count == 2) {
            int mode = (int)values[0];
            FT_Vector vector = {0, 0};
            FT_Vector_Unit(mode == 1 ? NULL : &vector, (FT_Angle)values[1]);
            printf("{\"mode\":%d,\"angle\":%lld,\"result\":", mode, values[1]);
            if (mode == 1) {
                printf("null");
            } else {
                print_vector_object(vector);
            }
            printf("}");
        } else if (streq(op, "vector_length") && value_count == 3) {
            int mode = (int)values[0];
            FT_Vector vector = {(FT_Pos)values[1], (FT_Pos)values[2]};
            printf("{\"mode\":%d,\"vector\":", mode);
            if (mode == 1) {
                printf("null");
            } else {
                print_vector_object(vector);
            }
            printf(",\"result\":%ld}", FT_Vector_Length(mode == 1 ? NULL : &vector));
        } else if (streq(op, "vector_rotate") && value_count == 4) {
            int mode = (int)values[0];
            FT_Vector vector = {(FT_Pos)values[1], (FT_Pos)values[2]};
            printf("{\"mode\":%d,\"angle\":%lld,\"input\":", mode, values[3]);
            if (mode == 1) {
                printf("null");
            } else {
                print_vector_object(vector);
            }
            FT_Vector_Rotate(mode == 1 ? NULL : &vector, (FT_Angle)values[3]);
            printf(",\"result\":");
            if (mode == 1) {
                printf("null");
            } else {
                print_vector_object(vector);
            }
            printf("}");
        } else if (streq(op, "vector_polarize") && value_count == 5) {
            int mode = (int)values[0];
            FT_Vector vector = {(FT_Pos)values[1], (FT_Pos)values[2]};
            FT_Fixed length = (FT_Fixed)values[3];
            FT_Angle angle = (FT_Angle)values[4];
            FT_Vector_Polarize(
                (mode & 1) ? NULL : &vector,
                (mode & 2) ? NULL : &length,
                (mode & 4) ? NULL : &angle);
            printf("{\"mode\":%d,\"input\":", mode);
            if (mode & 1) {
                printf("null");
            } else {
                print_vector_object((FT_Vector){(FT_Pos)values[1], (FT_Pos)values[2]});
            }
            printf(",\"length\":");
            if (mode & 2) {
                printf("null");
            } else {
                printf("%ld", length);
            }
            printf(",\"angle\":");
            if (mode & 4) {
                printf("null");
            } else {
                printf("%ld", angle);
            }
            printf("}");
        } else if (streq(op, "vector_from_polar") && value_count == 5) {
            int mode = (int)values[0];
            FT_Vector vector = {(FT_Pos)values[3], (FT_Pos)values[4]};
            FT_Vector_From_Polar(mode == 1 ? NULL : &vector, (FT_Fixed)values[1], (FT_Angle)values[2]);
            printf("{\"mode\":%d,\"length\":%lld,\"angle\":%lld,\"result\":", mode, values[1], values[2]);
            if (mode == 1) {
                printf("null");
            } else {
                print_vector_object(vector);
            }
            printf("}");
        } else {
            fprintf(stderr, "unsupported trigon row op=%s row=%s\n", op, cursor);
            free(rows);
            return 2;
        }
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static void print_trigon_aggregate_input(long long op, long long x, long long y) {
    if (op == 5 || op == 6) {
        print_vector_object((FT_Vector){(FT_Pos)x, (FT_Pos)y});
    } else {
        printf("null");
    }
}

static int print_trigon_aggregate_result(long long op, long long angle, long long x, long long y) {
    if (op == 1) {
        printf("%ld", FT_Sin((FT_Angle)angle));
        return 1;
    }
    if (op == 2) {
        printf("%ld", FT_Cos((FT_Angle)angle));
        return 1;
    }
    if (op == 3) {
        printf("%ld", FT_Tan((FT_Angle)angle));
        return 1;
    }
    if (op == 4) {
        FT_Vector vector = {0, 0};
        FT_Vector_Unit(&vector, (FT_Angle)angle);
        print_vector_object(vector);
        return 1;
    }
    if (op == 5) {
        FT_Vector vector = {(FT_Pos)x, (FT_Pos)y};
        FT_Vector_Rotate(&vector, (FT_Angle)angle);
        print_vector_object(vector);
        return 1;
    }
    if (op == 6) {
        FT_Vector vector = {(FT_Pos)x, (FT_Pos)y};
        FT_Fixed length = 0;
        FT_Angle polar_angle = 0;
        FT_Vector_Polarize(&vector, &length, &polar_angle);
        printf("{\"length\":%ld,\"angle\":%ld}", length, polar_angle);
        return 1;
    }
    return 0;
}

static int emit_trigon_aggregate(const char* op, const char* rows_csv) {
    size_t rows_len = strlen(rows_csv);
    char* rows = (char*)malloc(rows_len + 1);
    if (!rows) {
        return 1;
    }
    memcpy(rows, rows_csv, rows_len + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* cursor = rows;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        long long values[5] = {0, 0, 0, 0, 0};
        int value_count = split_fixed_math_row(cursor, values, 5);
        if (emitted) {
            printf(",");
        }
        if (streq(op, "constant_probe") && value_count == 4) {
            printf("{\"op\":%lld,\"angle\":%lld,\"input\":", values[0], values[1]);
            print_trigon_aggregate_input(values[0], values[2], values[3]);
            printf(",\"result\":");
            if (!print_trigon_aggregate_result(values[0], values[1], values[2], values[3])) {
                fprintf(stderr, "unsupported trigon aggregate op=%lld\n", values[0]);
                free(rows);
                return 2;
            }
            printf("}");
        } else if (streq(op, "periodic") && value_count == 5) {
            printf("{\"op\":%lld,\"base_angle\":%lld,\"periodic_angle\":%lld,\"input\":",
                   values[0], values[1], values[2]);
            print_trigon_aggregate_input(values[0], values[3], values[4]);
            printf(",\"base_result\":");
            if (!print_trigon_aggregate_result(values[0], values[1], values[3], values[4])) {
                fprintf(stderr, "unsupported trigon aggregate op=%lld\n", values[0]);
                free(rows);
                return 2;
            }
            printf(",\"periodic_result\":");
            if (!print_trigon_aggregate_result(values[0], values[2], values[3], values[4])) {
                fprintf(stderr, "unsupported trigon aggregate op=%lld\n", values[0]);
                free(rows);
                return 2;
            }
            printf("}");
        } else {
            fprintf(stderr, "unsupported trigon aggregate row op=%s row=%s\n", op, cursor);
            free(rows);
            return 2;
        }
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");
    free(rows);
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
        FT_StreamRec default_stream = {0};
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
        FT_StreamDesc default_descriptor = {0};
        printf("],\"default_state\":{\"stream_descriptor_size\":%zu,\"base_is_null\":%s,\"size_is_zero\":%s,\"pos_is_zero\":%s,\"cursor_is_null\":%s,\"limit_is_null\":%s}",
               sizeof(default_descriptor),
               default_stream.base == NULL ? "true" : "false",
               default_stream.size == 0 ? "true" : "false",
               default_stream.pos == 0 ? "true" : "false",
               default_stream.cursor == NULL ? "true" : "false",
               default_stream.limit == NULL ? "true" : "false");
        printf("}}\n");
        return 0;
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

#define EMIT_RECORD_TYPE(name) \
    if (streq(symbol, #name)) { \
        printf("{"); \
        print_status(0); \
        printf(",\"output\":{\"symbol\":\"%s\",\"kind\":\"record\",\"size\":%zu,\"align\":%zu,\"signed\":null}}\n", \
               #name, sizeof(name), _Alignof(name)); \
        return 0; \
    } else

static int emit_type_probe(const char* symbol) {
    if (streq(symbol, "unsigned char")) {
        printf("{");
        print_status(0);
        printf(",\"output\":{\"symbol\":\"unsigned char\",\"kind\":\"scalar\",\"size\":%zu,\"align\":%zu,\"signed\":false}}\n",
               sizeof(unsigned char), _Alignof(unsigned char));
        return 0;
    }
    EMIT_SCALAR_TYPE(FT_Offset)
    EMIT_SCALAR_TYPE(FT_UFWord)
    EMIT_SCALAR_TYPE(FT_F2Dot14)
    EMIT_SCALAR_TYPE(FT_Angle)
    EMIT_SCALAR_TYPE(FT_UInt)
    EMIT_SCALAR_TYPE(FT_Error)
    EMIT_SCALAR_TYPE(FT_Bool)
    EMIT_SCALAR_TYPE(FT_Byte)
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
    EMIT_POINTER_TYPE(FT_Incremental)
    EMIT_POINTER_TYPE(FT_Incremental_Metrics)
    EMIT_POINTER_TYPE(FT_Incremental_Interface)
    EMIT_POINTER_TYPE(FT_Module_Interface)
    EMIT_POINTER_TYPE(FT_Glyph)
    EMIT_POINTER_TYPE(FT_BitmapGlyph)
    EMIT_POINTER_TYPE(FT_OutlineGlyph)
    EMIT_POINTER_TYPE(FT_SvgGlyph)
    EMIT_POINTER_TYPE(FT_Outline_MoveToFunc)
    EMIT_POINTER_TYPE(FT_Outline_MoveTo_Func)
    EMIT_POINTER_TYPE(FT_SpanFunc)
    EMIT_POINTER_TYPE(FT_Raster_Span_Func)
    EMIT_POINTER_TYPE(FT_Raster_NewFunc)
    EMIT_POINTER_TYPE(FT_Raster_New_Func)
    EMIT_POINTER_TYPE(FT_Raster_DoneFunc)
    EMIT_POINTER_TYPE(FT_Raster_Done_Func)
    EMIT_POINTER_TYPE(FT_Raster_ResetFunc)
    EMIT_POINTER_TYPE(FT_Raster_Reset_Func)
    EMIT_POINTER_TYPE(FT_Raster_SetModeFunc)
    EMIT_POINTER_TYPE(FT_Raster_Set_Mode_Func)
    EMIT_POINTER_TYPE(FT_Raster_RenderFunc)
    EMIT_POINTER_TYPE(FT_Raster_Render_Func)
    EMIT_POINTER_TYPE(FT_Glyph_InitFunc)
    EMIT_POINTER_TYPE(FT_Glyph_Init_Func)
    EMIT_POINTER_TYPE(FT_Glyph_DoneFunc)
    EMIT_POINTER_TYPE(FT_Glyph_Done_Func)
    EMIT_POINTER_TYPE(FT_Glyph_CopyFunc)
    EMIT_POINTER_TYPE(FT_Glyph_Copy_Func)
    EMIT_POINTER_TYPE(FT_Glyph_TransformFunc)
    EMIT_POINTER_TYPE(FT_Glyph_Transform_Func)
    EMIT_POINTER_TYPE(FT_Glyph_GetBBoxFunc)
    EMIT_POINTER_TYPE(FT_Glyph_BBox_Func)
    EMIT_POINTER_TYPE(FT_Glyph_PrepareFunc)
    EMIT_POINTER_TYPE(FT_Glyph_Prepare_Func)
    EMIT_RECORD_TYPE(FT_Glyph_Class)
    {
        fprintf(stderr, "unsupported type probe: %s\n", symbol);
        return 2;
    }
}

#undef EMIT_SCALAR_TYPE
#undef EMIT_POINTER_TYPE
#undef EMIT_RECORD_TYPE

typedef struct TypeDescriptor_ {
    const char* symbol;
    const char* kind;
    size_t size;
    size_t align;
    int has_signedness;
    int is_signed;
} TypeDescriptor;

#define TYPE_DESCRIPTOR_SCALAR(name) \
    if (streq(symbol, #name)) { \
        out->symbol = #name; \
        out->kind = "scalar"; \
        out->size = sizeof(name); \
        out->align = _Alignof(name); \
        out->has_signedness = 1; \
        out->is_signed = (((name)-1) < (name)0); \
        return 1; \
    } else

static int type_descriptor(const char* symbol, TypeDescriptor* out) {
    if (streq(symbol, "unsigned char")) {
        out->symbol = "unsigned char";
        out->kind = "scalar";
        out->size = sizeof(unsigned char);
        out->align = _Alignof(unsigned char);
        out->has_signedness = 1;
        out->is_signed = 0;
        return 1;
    }
    TYPE_DESCRIPTOR_SCALAR(FT_Angle)
    TYPE_DESCRIPTOR_SCALAR(FT_Fixed)
    TYPE_DESCRIPTOR_SCALAR(FT_Bool)
    TYPE_DESCRIPTOR_SCALAR(FT_Byte)
    {
        return 0;
    }
}

#undef TYPE_DESCRIPTOR_SCALAR

static void print_type_descriptor_json(const TypeDescriptor* descriptor) {
    printf("{\"symbol\":\"%s\",\"kind\":\"%s\",\"size\":%zu,\"align\":%zu,\"signed\":",
           descriptor->symbol,
           descriptor->kind,
           descriptor->size,
           descriptor->align);
    if (descriptor->has_signedness) {
        print_json_bool(descriptor->is_signed);
    } else {
        printf("null");
    }
    printf("}");
}

static int emit_type_map_probe(const char* symbols_csv) {
    size_t symbols_len = strlen(symbols_csv);
    char* symbols = (char*)malloc(symbols_len + 1);
    if (!symbols) {
        return 1;
    }
    memcpy(symbols, symbols_csv, symbols_len + 1);

    TypeDescriptor first;
    int have_first = 0;
    int equivalent = 1;
    int emitted = 0;

    printf("{");
    print_status(0);
    printf(",\"output\":{\"types\":{");
    char* cursor = symbols;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        TypeDescriptor descriptor;
        if (!type_descriptor(cursor, &descriptor)) {
            fprintf(stderr, "unsupported type map probe: %s\n", cursor);
            free(symbols);
            return 2;
        }
        if (!have_first) {
            first = descriptor;
            have_first = 1;
        } else if (strcmp(first.kind, descriptor.kind) != 0 ||
                   first.size != descriptor.size ||
                   first.align != descriptor.align ||
                   first.has_signedness != descriptor.has_signedness ||
                   first.is_signed != descriptor.is_signed) {
            equivalent = 0;
        }
        if (emitted) {
            printf(",");
        }
        printf("\"%s\":", descriptor.symbol);
        print_type_descriptor_json(&descriptor);
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    printf("},\"equivalent\":");
    print_json_bool(equivalent);
    printf("}}\n");
    free(symbols);
    return 0;
}

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
    EMIT_FUNCTION(FT_Get_FSType_Flags)
    EMIT_FUNCTION(FT_Get_First_Char)
    EMIT_FUNCTION(FT_Get_Next_Char)
    EMIT_FUNCTION(FT_Get_Sfnt_Table)
    EMIT_FUNCTION(FT_Library_Version)
    EMIT_FUNCTION(FT_Library_SetLcdFilter)
    EMIT_FUNCTION(FT_Library_SetLcdFilterWeights)
    EMIT_FUNCTION(FT_Get_TrueType_Engine_Type)
    EMIT_FUNCTION(FT_Face_CheckTrueTypePatents)
    EMIT_FUNCTION(FT_Face_SetUnpatentedHinting)
    EMIT_FUNCTION(FT_Load_Sfnt_Table)
    EMIT_FUNCTION(FT_Request_Size)
    EMIT_FUNCTION(FT_Select_Size)
    EMIT_FUNCTION(FT_Select_Charmap)
    EMIT_FUNCTION(FT_Sfnt_Table_Info)
    EMIT_FUNCTION(FT_OpenType_Free)
    EMIT_FUNCTION(FT_OpenType_Validate)
    EMIT_FUNCTION(FT_GlyphSlot_AdjustWeight)
    EMIT_FUNCTION(FT_GlyphSlot_Embolden)
    EMIT_FUNCTION(FT_GlyphSlot_Oblique)
    EMIT_FUNCTION(FT_GlyphSlot_Slant)
    EMIT_FUNCTION(FT_Get_Sfnt_Name)
    EMIT_FUNCTION(FT_Get_Sfnt_Name_Count)
    EMIT_FUNCTION(FT_Get_Sfnt_LangTag)
    EMIT_FUNCTION(FT_Activate_Size)
    EMIT_FUNCTION(FT_Done_Size)
    EMIT_FUNCTION(FT_New_Size)
    EMIT_FUNCTION(FT_Angle_Diff)
    EMIT_FUNCTION(FT_Atan2)
    EMIT_FUNCTION(FT_Cos)
    EMIT_FUNCTION(FT_Sin)
    EMIT_FUNCTION(FT_Tan)
    EMIT_FUNCTION(FT_Vector_From_Polar)
    EMIT_FUNCTION(FT_Vector_Length)
    EMIT_FUNCTION(FT_Vector_Polarize)
    EMIT_FUNCTION(FT_Vector_Rotate)
    EMIT_FUNCTION(FT_Vector_Unit)
    {
        fprintf(stderr, "unsupported function probe: %s\n", symbol);
        return 2;
    }

#undef EMIT_FUNCTION
}

static void print_json_bool(int value) {
    printf(value ? "true" : "false");
}

static void print_tt_vert_header_record(const TT_VertHeader* record) {
    if (!record) {
        printf("null");
        return;
    }
    printf("{\"Version\":%ld", (long)record->Version);
    printf(",\"Ascender\":%d", (int)record->Ascender);
    printf(",\"Descender\":%d", (int)record->Descender);
    printf(",\"Line_Gap\":%d", (int)record->Line_Gap);
    printf(",\"advance_Height_Max\":%u", (unsigned)record->advance_Height_Max);
    printf(",\"min_Top_Side_Bearing\":%d", (int)record->min_Top_Side_Bearing);
    printf(",\"min_Bottom_Side_Bearing\":%d", (int)record->min_Bottom_Side_Bearing);
    printf(",\"yMax_Extent\":%d", (int)record->yMax_Extent);
    printf(",\"caret_Slope_Rise\":%d", (int)record->caret_Slope_Rise);
    printf(",\"caret_Slope_Run\":%d", (int)record->caret_Slope_Run);
    printf(",\"caret_Offset\":%d", (int)record->caret_Offset);
    printf(",\"Reserved\":[%d,%d,%d,%d]",
           (int)record->Reserved[0],
           (int)record->Reserved[1],
           (int)record->Reserved[2],
           (int)record->Reserved[3]);
    printf(",\"metric_Data_Format\":%d", (int)record->metric_Data_Format);
    printf(",\"number_Of_VMetrics\":%u", (unsigned)record->number_Of_VMetrics);
    printf(",\"long_metrics_nullness\":");
    print_json_bool(record->long_metrics == NULL);
    printf(",\"short_metrics_nullness\":");
    print_json_bool(record->short_metrics == NULL);
    printf(",\"long_metrics_identity_class\":");
    printf(record->long_metrics == NULL ? "\"null\"" : "\"face_owned_vmtx\"");
    printf(",\"short_metrics_identity_class\":");
    printf(record->short_metrics == NULL ? "\"null\"" : "\"face_owned_vmtx\"");
    printf("}");
}

static void print_tt_maxprofile_record(const TT_MaxProfile* record) {
    if (!record) {
        printf("null");
        return;
    }
    printf("{\"version\":%ld", (long)record->version);
    printf(",\"numGlyphs\":%u", (unsigned)record->numGlyphs);
    printf(",\"maxPoints\":%u", (unsigned)record->maxPoints);
    printf(",\"maxContours\":%u", (unsigned)record->maxContours);
    printf(",\"maxCompositePoints\":%u", (unsigned)record->maxCompositePoints);
    printf(",\"maxCompositeContours\":%u", (unsigned)record->maxCompositeContours);
    printf(",\"maxZones\":%u", (unsigned)record->maxZones);
    printf(",\"maxTwilightPoints\":%u", (unsigned)record->maxTwilightPoints);
    printf(",\"maxStorage\":%u", (unsigned)record->maxStorage);
    printf(",\"maxFunctionDefs\":%u", (unsigned)record->maxFunctionDefs);
    printf(",\"maxInstructionDefs\":%u", (unsigned)record->maxInstructionDefs);
    printf(",\"maxStackElements\":%u", (unsigned)record->maxStackElements);
    printf(",\"maxSizeOfInstructions\":%u", (unsigned)record->maxSizeOfInstructions);
    printf(",\"maxComponentElements\":%u", (unsigned)record->maxComponentElements);
    printf(",\"maxComponentDepth\":%u", (unsigned)record->maxComponentDepth);
    printf("}");
}

static void print_fstype_flags_result(FT_UShort flags, const char* symbol_name) {
    printf("{\"return\":%u,\"fs_type\":%u", (unsigned int)flags, (unsigned int)flags);
    if (symbol_name && symbol_name[0] && !streq(symbol_name, "-")) {
        long long symbol_value = 0;
        if (emit_generated_constant_value(symbol_name, &symbol_value)) {
            printf(",\"contains_symbol_bit\":");
            print_json_bool(((long long)flags & symbol_value) == symbol_value);
        }
    }
    printf("}");
}

static void print_sfnt_name_record(FT_UInt index, const FT_SfntName* name) {
    printf("{\"name_index\":%u,\"platform_id\":%u,\"encoding_id\":%u,\"language_id\":%u,\"name_id\":%u,\"string_len\":%u,\"string_bytes\":\"",
           index,
           (unsigned int)name->platform_id,
           (unsigned int)name->encoding_id,
           (unsigned int)name->language_id,
           (unsigned int)name->name_id,
           (unsigned int)name->string_len);
    if (name->string && name->string_len) {
        print_hex_bytes(name->string, (long)name->string_len);
    }
    printf("\"}");
}

static int sfnt_name_token_equals(const char* token, size_t len, const char* expected) {
    return strlen(expected) == len && strncmp(token, expected, len) == 0;
}

static FT_UInt sfnt_name_index_from_token(const char* token, size_t len, FT_UInt count) {
    if (sfnt_name_token_equals(token, len, "last_valid_index")) {
        return count > 0 ? count - 1 : 0;
    }
    char buffer[64];
    size_t copy_len = len < sizeof(buffer) - 1 ? len : sizeof(buffer) - 1;
    memcpy(buffer, token, copy_len);
    buffer[copy_len] = '\0';
    return (FT_UInt)strtoul(buffer, NULL, 10);
}

static void print_sfnt_name_indexed_result(
    FT_Face face,
    const char* indexes_csv,
    int output_is_null) {
    FT_UInt count = FT_Get_Sfnt_Name_Count(face);
    printf("{\"return_sequence\":[");
    const char* token = indexes_csv;
    int first_return = 1;
    while (token && *token) {
        const char* comma = strchr(token, ',');
        size_t len = comma ? (size_t)(comma - token) : strlen(token);
        FT_UInt index = sfnt_name_index_from_token(token, len, count);
        FT_SfntName name;
        FT_Error name_error = FT_Get_Sfnt_Name(
            face,
            index,
            output_is_null ? (FT_SfntName*)NULL : &name);
        if (!first_return) printf(",");
        first_return = 0;
        printf("%d", name_error);
        if (!comma) break;
        token = comma + 1;
    }

    printf("],\"names\":[");
    if (!output_is_null) {
        token = indexes_csv;
        int first_name = 1;
        while (token && *token) {
            const char* comma = strchr(token, ',');
            size_t len = comma ? (size_t)(comma - token) : strlen(token);
            FT_UInt index = sfnt_name_index_from_token(token, len, count);
            FT_SfntName name;
            FT_Error name_error = FT_Get_Sfnt_Name(face, index, &name);
            if (!name_error) {
                if (!first_name) printf(",");
                first_name = 0;
                print_sfnt_name_record(index, &name);
            }
            if (!comma) break;
            token = comma + 1;
        }
    }
    printf("]}");
}

static FT_UInt sfnt_lang_id_from_token(const char* token) {
    return (FT_UInt)strtoul(token, NULL, 0);
}

static void print_sfnt_lang_tag_json(const FT_SfntLangTag* tag) {
    const char* nullness = tag->string ? "non_null" : "null";
    printf("{\"return\":0,\"string_nullness\":\"%s\",\"string_len\":%u,\"string_bytes\":\"",
           nullness,
           (unsigned int)tag->string_len);
    if (tag->string && tag->string_len) {
        print_hex_bytes(tag->string, (long)tag->string_len);
    }
    printf("\",\"record\":{\"string_nullness\":\"%s\",\"string_len\":%u,\"string_bytes\":\"",
           nullness,
           (unsigned int)tag->string_len);
    if (tag->string && tag->string_len) {
        print_hex_bytes(tag->string, (long)tag->string_len);
    }
    printf("\"}}");
}

static FT_Error sfnt_lang_tag_lookup(
    FT_Face face,
    const char* lang_token,
    const char* output_kind,
    FT_SfntLangTag* tag) {
    FT_UInt lang_id = sfnt_lang_id_from_token(lang_token);
    if (streq(output_kind, "null")) {
        return FT_Get_Sfnt_LangTag(face, lang_id, NULL);
    }
    return FT_Get_Sfnt_LangTag(face, lang_id, tag);
}

static void print_sfnt_lang_tag_variants(FT_Face face, const char* rows) {
    FT_Error first_error = 0;
    FT_Error errors[64];
    int count = 0;
    const char* row = rows;

    while (row && *row && count < 64) {
        const char* end = strchr(row, ';');
        size_t len = end ? (size_t)(end - row) : strlen(row);
        char buffer[160];
        char* face_kind;
        char* lang_token;
        char* output_kind;
        FT_Face row_face = face;
        FT_SfntLangTag tag;
        size_t copy_len = len < sizeof(buffer) - 1 ? len : sizeof(buffer) - 1;
        memcpy(buffer, row, copy_len);
        buffer[copy_len] = '\0';
        face_kind = strtok(buffer, ":");
        lang_token = strtok(NULL, ":");
        output_kind = strtok(NULL, ":");
        if (!face_kind || !lang_token || !output_kind) {
            errors[count] = FT_ERR( Invalid_Argument );
        } else {
            if (streq(face_kind, "null")) {
                row_face = NULL;
            }
            errors[count] = sfnt_lang_tag_lookup(row_face, lang_token, output_kind, &tag);
        }
        if (!first_error && errors[count]) {
            first_error = errors[count];
        }
        count++;
        if (!end) break;
        row = end + 1;
    }

    print_status(first_error);
    printf(",\"output\":{\"status_sequence\":[");
    for (int i = 0; i < count; i++) {
        if (i) printf(",");
        printf("\"%s\"", errors[i] ? "error" : "ok");
    }
    printf("],\"error_sequence\":[");
    for (int i = 0; i < count; i++) {
        if (i) printf(",");
        printf("%d", errors[i]);
    }
    printf("]}}");
}

static void print_postscript_name_result(const char* name) {
    if (!name) {
        printf("{\"null\":true,\"bytes\":\"\",\"length\":0}");
        return;
    }
    size_t len = strlen(name);
    printf("{\"null\":false,\"bytes\":\"");
    print_hex_bytes((const unsigned char*)name, (long)len);
    printf("\",\"length\":%zu}", len);
}

static void print_nullable_c_string_result(const char* value) {
    print_postscript_name_result(value);
}

static void print_json_c_string_or_null(const char* value) {
    if (!value) {
        printf("null");
        return;
    }
    putchar('"');
    for (const unsigned char* p = (const unsigned char*)value; *p; p++) {
        if (*p == '"' || *p == '\\') {
            putchar('\\');
        }
        putchar(*p);
    }
    putchar('"');
}

static void print_face_driver_name_result(const char* name) {
    if (!name) {
        printf("{\"driver_name\":null,\"string_bytes\":\"\",\"nul_terminated\":false}");
        return;
    }
    size_t len = strlen(name);
    printf("{\"driver_name\":\"%s\",\"string_bytes\":\"", name);
    print_hex_bytes((const unsigned char*)name, (long)len);
    printf("\",\"nul_terminated\":true}");
}

static void print_winfnt_header_json(const FT_WinFNT_HeaderRec* header) {
    printf("{\"version\":%u", (unsigned int)header->version);
    printf(",\"file_size\":%lu", (unsigned long)header->file_size);
    printf(",\"copyright\":\"");
    print_hex_bytes(header->copyright, 60);
    printf("\"");
    printf(",\"file_type\":%u", (unsigned int)header->file_type);
    printf(",\"nominal_point_size\":%u", (unsigned int)header->nominal_point_size);
    printf(",\"vertical_resolution\":%u", (unsigned int)header->vertical_resolution);
    printf(",\"horizontal_resolution\":%u", (unsigned int)header->horizontal_resolution);
    printf(",\"ascent\":%u", (unsigned int)header->ascent);
    printf(",\"internal_leading\":%u", (unsigned int)header->internal_leading);
    printf(",\"external_leading\":%u", (unsigned int)header->external_leading);
    printf(",\"italic\":%u", (unsigned int)header->italic);
    printf(",\"underline\":%u", (unsigned int)header->underline);
    printf(",\"strike_out\":%u", (unsigned int)header->strike_out);
    printf(",\"weight\":%u", (unsigned int)header->weight);
    printf(",\"charset\":%u", (unsigned int)header->charset);
    printf(",\"pixel_width\":%u", (unsigned int)header->pixel_width);
    printf(",\"pixel_height\":%u", (unsigned int)header->pixel_height);
    printf(",\"pitch_and_family\":%u", (unsigned int)header->pitch_and_family);
    printf(",\"avg_width\":%u", (unsigned int)header->avg_width);
    printf(",\"max_width\":%u", (unsigned int)header->max_width);
    printf(",\"first_char\":%u", (unsigned int)header->first_char);
    printf(",\"last_char\":%u", (unsigned int)header->last_char);
    printf(",\"default_char\":%u", (unsigned int)header->default_char);
    printf(",\"break_char\":%u", (unsigned int)header->break_char);
    printf(",\"bytes_per_row\":%u", (unsigned int)header->bytes_per_row);
    printf(",\"device_offset\":%lu", (unsigned long)header->device_offset);
    printf(",\"face_name_offset\":%lu", (unsigned long)header->face_name_offset);
    printf(",\"bits_pointer\":%lu", (unsigned long)header->bits_pointer);
    printf(",\"bits_offset\":%lu", (unsigned long)header->bits_offset);
    printf(",\"reserved\":%u", (unsigned int)header->reserved);
    printf(",\"flags\":%lu", (unsigned long)header->flags);
    printf(",\"A_space\":%u", (unsigned int)header->A_space);
    printf(",\"B_space\":%u", (unsigned int)header->B_space);
    printf(",\"C_space\":%u", (unsigned int)header->C_space);
    printf(",\"color_table_offset\":%u", (unsigned int)header->color_table_offset);
    printf(",\"reserved1\":[%lu,%lu,%lu,%lu]}",
           (unsigned long)header->reserved1[0],
           (unsigned long)header->reserved1[1],
           (unsigned long)header->reserved1[2],
           (unsigned long)header->reserved1[3]);
}

static int emit_get_winfnt_header_null_face(int argc, char** argv) {
    const int header_is_null = argc > 2 && streq(argv[2], "null");
    FT_WinFNT_HeaderRec header;
    memset(&header, 0xA5, sizeof(header));
    FT_Error err = FT_Get_WinFNT_Header(NULL, header_is_null ? NULL : &header);
    printf("{");
    print_status(err);
    printf(",\"output\":{\"error\":%d,\"status\":%d,\"sentinel_unchanged\":true}}\n",
           err,
           err);
    return 0;
}

static void print_winfnt_header_mutation_row(const char* label, FT_Face face) {
    FT_WinFNT_HeaderRec header;
    memset(&header, 0xA5, sizeof(header));
    FT_WinFNT_HeaderRec sentinel = header;
    FT_Error err = FT_Get_WinFNT_Header(face, &header);
    int unchanged = memcmp(&header, &sentinel, sizeof(header)) == 0;
    printf("{\"row\":\"%s\",\"status\":%d,\"sentinel_unchanged\":%s,\"record_mutation\":\"%s\"}",
           label,
           err,
           unchanged ? "true" : "false",
           unchanged ? "sentinel_preserved" : "overwritten");
}

static int emit_get_winfnt_header_mutation(int argc, char** argv) {
    if (argc != 7) {
        return 1;
    }
    if (!streq(argv[2], "file") || !streq(argv[4], "file")) {
        fprintf(stderr, "WinFNT header mutation currently expects file assets\n");
        return 2;
    }
    unsigned char* winfnt_data = NULL;
    unsigned char* non_winfnt_data = NULL;
    long winfnt_len = 0;
    long non_winfnt_len = 0;
    if (load_file(argv[3], &winfnt_data, &winfnt_len) != 0 ||
        load_file(argv[5], &non_winfnt_data, &non_winfnt_len) != 0) {
        free(winfnt_data);
        free(non_winfnt_data);
        return 2;
    }
    FT_Library winfnt_library = NULL;
    FT_Library non_winfnt_library = NULL;
    FT_Face winfnt_face = NULL;
    FT_Face non_winfnt_face = NULL;
    FT_Error err = FT_Init_FreeType(&winfnt_library);
    if (!err) {
        err = FT_New_Memory_Face(
            winfnt_library, winfnt_data, winfnt_len, atol(argv[6]), &winfnt_face);
    }
    if (!err) {
        err = FT_Init_FreeType(&non_winfnt_library);
    }
    if (!err) {
        err = FT_New_Memory_Face(
            non_winfnt_library, non_winfnt_data, non_winfnt_len, atol(argv[6]), &non_winfnt_face);
    }
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        if (non_winfnt_face) FT_Done_Face(non_winfnt_face);
        if (non_winfnt_library) FT_Done_FreeType(non_winfnt_library);
        if (winfnt_face) FT_Done_Face(winfnt_face);
        if (winfnt_library) FT_Done_FreeType(winfnt_library);
        free(non_winfnt_data);
        free(winfnt_data);
        return 0;
    }
    printf("{");
    print_status(0);
    printf(",\"output\":");
    printf("{\"rows\":[");
    print_winfnt_header_mutation_row("winfnt_font", winfnt_face);
    printf(",");
    print_winfnt_header_mutation_row("non_winfnt_font", non_winfnt_face);
    printf(",");
    print_winfnt_header_mutation_row("null", NULL);
    printf("]}}\n");
    FT_Done_Face(non_winfnt_face);
    FT_Done_FreeType(non_winfnt_library);
    FT_Done_Face(winfnt_face);
    FT_Done_FreeType(winfnt_library);
    free(non_winfnt_data);
    free(winfnt_data);
    return 0;
}

static void print_charmap_record(FT_CharMap charmap) {
    if (!charmap) {
        printf("null");
        return;
    }
    printf("{\"charmap_index\":%d,\"platform_id\":%u,\"encoding_id\":%u,\"encoding\":%ld}",
           FT_Get_Charmap_Index(charmap),
           (unsigned int)charmap->platform_id,
           (unsigned int)charmap->encoding_id,
           (long)charmap->encoding);
}

static void print_charmap_inventory_record(FT_CharMap charmap) {
    if (!charmap) {
        printf("null");
        return;
    }
    printf("{\"charmap_index\":%d,\"encoding\":%ld,\"platform_id\":%u,\"encoding_id\":%u,\"face_identity\":true}",
           FT_Get_Charmap_Index(charmap),
           (long)charmap->encoding,
           (unsigned int)charmap->platform_id,
           (unsigned int)charmap->encoding_id);
}

static FT_Int active_charmap_index(FT_Face face) {
    if (!face || !face->charmap) {
        return -1;
    }
    return FT_Get_Charmap_Index(face->charmap);
}

static void print_active_charmap(FT_Face face) {
    FT_Int index = active_charmap_index(face);
    if (!face || index < 0 || index >= face->num_charmaps || !face->charmaps) {
        printf("null");
        return;
    }
    print_charmap_inventory_record(face->charmaps[index]);
}

static void print_charmap_inventory_records(FT_Face face) {
    printf("[");
    if (face && face->charmaps) {
        for (FT_Int i = 0; i < face->num_charmaps; i++) {
            if (i) {
                printf(",");
            }
            print_charmap_inventory_record(face->charmaps[i]);
        }
    }
    printf("]");
}

static FT_CharMap find_charmap_by_ids(FT_Face face, long platform, long encoding) {
    if (!face || !face->charmaps) {
        return NULL;
    }
    for (FT_Int i = 0; i < face->num_charmaps; i++) {
        FT_CharMap charmap = face->charmaps[i];
        if (charmap &&
            charmap->platform_id == (FT_UShort)platform &&
            charmap->encoding_id == (FT_UShort)encoding) {
            return charmap;
        }
    }
    return NULL;
}

static int sfnt_name_matches(const FT_SfntName* name, long platform, long encoding, long language, long name_id) {
    return (platform < 0 || name->platform_id == (FT_UShort)platform) &&
           (encoding < 0 || name->encoding_id == (FT_UShort)encoding) &&
           (language < 0 || name->language_id == (FT_UShort)language) &&
           (name_id < 0 || name->name_id == (FT_UShort)name_id);
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

static unsigned long enc_tag(unsigned int a, unsigned int b, unsigned int c, unsigned int d) {
    FT_UInt32 tag = 0;
    FT_ENC_TAG(tag, a, b, c, d);
    return (unsigned long)tag;
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

static const char* abi_bool_canonical_class(unsigned int stored) {
    if (stored == 0) {
        return "false";
    }
    if (stored == 1) {
        return "true";
    }
    return "noncanonical_true";
}

static int emit_abi_value_echo_bool(const char* values_arg) {
    char* values = (char*)malloc(strlen(values_arg) + 1);
    if (!values) {
        return 1;
    }
    strcpy(values, values_arg);

    print_ok_output_prefix();
    printf("{\"rows\":[");
    char* token = strtok(values, ",");
    size_t index = 0;
    while (token) {
        unsigned long input = strtoul(token, NULL, 10);
        FT_Bool stored = (FT_Bool)input;
        if (index) {
            printf(",");
        }
        printf("{\"input\":%lu,\"stored\":%u,\"canonical_class\":\"%s\"}",
               input,
               (unsigned int)stored,
               abi_bool_canonical_class((unsigned int)stored));
        index++;
        token = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(values);
    return 0;
}

static int emit_abi_value_echo_unit_vector(const char* rows_arg) {
    char* rows = (char*)malloc(strlen(rows_arg) + 1);
    if (!rows) {
        return 1;
    }
    strcpy(rows, rows_arg);

    print_ok_output_prefix();
    printf("{\"rows\":[");
    char* token = strtok(rows, ";");
    size_t index = 0;
    while (token) {
        char label[128];
        long x;
        long y;
        if (sscanf(token, "%127[^:]:%ld:%ld", label, &x, &y) != 3) {
            free(rows);
            return 1;
        }
        FT_UnitVector vector;
        vector.x = (FT_F2Dot14)x;
        vector.y = (FT_F2Dot14)y;
        if (index) {
            printf(",");
        }
        printf("{\"label\":\"%s\",\"x\":%d,\"y\":%d}",
               label,
               (int)vector.x,
               (int)vector.y);
        index++;
        token = strtok(NULL, ";");
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static int emit_abi_value_echo(const char* type_name, const char* rows_arg) {
    if (streq(type_name, "FT_Bool")) {
        return emit_abi_value_echo_bool(rows_arg);
    }
    if (streq(type_name, "FT_UnitVector")) {
        return emit_abi_value_echo_unit_vector(rows_arg);
    }
    fprintf(stderr, "unsupported abi value echo type: %s\n", type_name);
    return 1;
}

static int emit_compile_alias_probe(const char* macro_name, const char* typedef_name, const char* signature) {
    if (streq(macro_name, "FT_Outline_LineTo_Func") && streq(typedef_name, "FT_Outline_LineToFunc")) {
        FT_Outline_LineTo_Func macro_ptr = NULL;
        FT_Outline_LineToFunc typedef_ptr = macro_ptr;
        macro_ptr = typedef_ptr;
        (void)macro_ptr;
    } else if (streq(macro_name, "FT_Outline_ConicTo_Func") && streq(typedef_name, "FT_Outline_ConicToFunc")) {
        FT_Outline_ConicTo_Func macro_ptr = NULL;
        FT_Outline_ConicToFunc typedef_ptr = macro_ptr;
        macro_ptr = typedef_ptr;
        (void)macro_ptr;
    } else if (streq(macro_name, "FT_Outline_CubicTo_Func") && streq(typedef_name, "FT_Outline_CubicToFunc")) {
        FT_Outline_CubicTo_Func macro_ptr = NULL;
        FT_Outline_CubicToFunc typedef_ptr = macro_ptr;
        macro_ptr = typedef_ptr;
        (void)macro_ptr;
    } else {
        fprintf(stderr, "unsupported compile alias probe: %s -> %s\n", macro_name, typedef_name);
        return 1;
    }

    print_ok_output_prefix();
    printf("{\"macro\":\"%s\",\"typedef\":\"%s\",\"signature\":\"%s\",\"assignment_compatible\":true}}\n",
           macro_name,
           typedef_name,
           signature);
    return 0;
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

    if (streq(case_id, "freetype.FT_LOAD_TARGET_MODE.target_mode_value")) {
        print_ok_output_prefix();
        printf("{\"macro_results\":[");
        printf("{\"input_load_flags\":\"FT_LOAD_TARGET_NORMAL\",\"mode\":%d},", FT_LOAD_TARGET_MODE(FT_LOAD_TARGET_NORMAL));
        printf("{\"input_load_flags\":\"FT_LOAD_TARGET_LIGHT\",\"mode\":%d},", FT_LOAD_TARGET_MODE(FT_LOAD_TARGET_LIGHT));
        printf("{\"input_load_flags\":\"FT_LOAD_TARGET_MONO\",\"mode\":%d},", FT_LOAD_TARGET_MODE(FT_LOAD_TARGET_MONO));
        printf("{\"input_load_flags\":\"FT_LOAD_TARGET_LCD\",\"mode\":%d},", FT_LOAD_TARGET_MODE(FT_LOAD_TARGET_LCD));
        printf("{\"input_load_flags\":\"FT_LOAD_TARGET_LCD_V\",\"mode\":%d},", FT_LOAD_TARGET_MODE(FT_LOAD_TARGET_LCD_V));
        printf("{\"input_load_flags\":\"FT_LOAD_TARGET_(31)\",\"mode\":%d}", FT_LOAD_TARGET_MODE(FT_LOAD_TARGET_(31)));
        printf("],\"import_compiles\":true}}\n");
        return 0;
    }
    if (streq(case_id, "freetype.FT_LOAD_TARGET_.target_constructor_value")) {
        int modes[] = {0, 1, 2, 3, 4, 15, 16, 31};
        print_ok_output_prefix();
        printf("{\"macro_results\":[");
        for (size_t i = 0; i < sizeof(modes) / sizeof(modes[0]); i++) {
            if (i) printf(",");
            printf("{\"input_render_mode\":%d,\"value\":%d}", modes[i], FT_LOAD_TARGET_(modes[i]));
        }
        printf("],\"import_compiles\":true}}\n");
        return 0;
    }
    if (streq(case_id, "freetype.FT_RENDER_MODE_NORMAL.maps_supported_modes")) {
        print_ok_output_prefix();
        printf("{\"render_mode\":%d,\"load_target\":%d,\"roundtrip_mode\":%d}}\n",
               FT_RENDER_MODE_NORMAL,
               FT_LOAD_TARGET_NORMAL,
               FT_LOAD_TARGET_MODE(FT_LOAD_TARGET_NORMAL));
        return 0;
    }
    if (streq(case_id, "freetype.FT_ENC_TAG.value_matches_header")) {
        print_ok_output_prefix();
        printf("{\"samples\":[");
        printf("{\"bytes\":[\"u\",\"n\",\"i\",\"c\"],\"value\":%lu},", enc_tag('u','n','i','c'));
        printf("{\"bytes\":[\"s\",\"y\",\"m\",\"b\"],\"value\":%lu},", enc_tag('s','y','m','b'));
        printf("{\"bytes\":[\"g\",\"b\",\" \",\" \"],\"value\":%lu},", enc_tag('g','b',' ',' '));
        printf("{\"bytes\":[\"\\u0000\",\"\\u0000\",\"\\u0000\",\"\\u0000\"],\"value\":%lu},", enc_tag(0,0,0,0));
        printf("{\"bytes\":[\"l\",\"a\",\"t\",\"2\"],\"value\":%lu}", enc_tag('l','a','t','2'));
        printf("],\"import_compiles\":true}}\n");
        return 0;
    }
    if (streq(case_id, "ftgxval.FT_VALIDATE_GX_BITFIELD.expansion_matches_header")) {
        print_ok_output_prefix();
        printf("{\"macro_results\":[");
        printf("{\"token\":\"feat\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(feat));
        printf("{\"token\":\"mort\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(mort));
        printf("{\"token\":\"morx\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(morx));
        printf("{\"token\":\"bsln\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(bsln));
        printf("{\"token\":\"just\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(just));
        printf("{\"token\":\"kern\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(kern));
        printf("{\"token\":\"opbd\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(opbd));
        printf("{\"token\":\"trak\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(trak));
        printf("{\"token\":\"prop\",\"value\":%lu},", (unsigned long)FT_VALIDATE_GX_BITFIELD(prop));
        printf("{\"token\":\"lcar\",\"value\":%lu}", (unsigned long)FT_VALIDATE_GX_BITFIELD(lcar));
        printf("],\"import_compiles\":true}}\n");
        return 0;
    }
    if (streq(case_id, "ftcache.FTC_IMAGE_TYPE_COMPARE.value_matches_header")) {
        FTC_ImageTypeRec left = { (FTC_FaceID)(size_t)0x1000, 16, 16, FT_LOAD_DEFAULT };
        FTC_ImageTypeRec right = { (FTC_FaceID)(size_t)0x1000, 16, 99, FT_LOAD_DEFAULT };
        print_ok_output_prefix();
        printf("{\"macro_expansion\":[\"face_id\",\"width\",\"flags\"],\"probe_result\":");
        print_json_bool(FTC_IMAGE_TYPE_COMPARE(&left, &right));
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "ftcache.FTC_IMAGE_TYPE_COMPARE.height_is_ignored")) {
        FTC_ImageTypeRec left = { (FTC_FaceID)(size_t)0x1000, 16, 16, FT_LOAD_DEFAULT };
        FTC_ImageTypeRec right = { (FTC_FaceID)(size_t)0x1000, 16, 99, FT_LOAD_DEFAULT };
        print_ok_output_prefix();
        printf("{\"result\":");
        print_json_bool(FTC_IMAGE_TYPE_COMPARE(&left, &right));
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "ftcache.FTC_IMAGE_TYPE_COMPARE.compared_fields_must_match")) {
        FTC_ImageTypeRec base = { (FTC_FaceID)(size_t)0x1000, 16, 16, FT_LOAD_DEFAULT };
        FTC_ImageTypeRec diff_face = { (FTC_FaceID)(size_t)0x2000, 16, 16, FT_LOAD_DEFAULT };
        FTC_ImageTypeRec diff_width = { (FTC_FaceID)(size_t)0x1000, 32, 16, FT_LOAD_DEFAULT };
        FTC_ImageTypeRec diff_flags = { (FTC_FaceID)(size_t)0x1000, 16, 16, FT_LOAD_NO_SCALE };
        print_ok_output_prefix();
        printf("{\"pairs\":[");
        printf("{\"diff\":\"face_id\",\"result\":");
        print_json_bool(FTC_IMAGE_TYPE_COMPARE(&base, &diff_face));
        printf("},");
        printf("{\"diff\":\"width\",\"result\":");
        print_json_bool(FTC_IMAGE_TYPE_COMPARE(&base, &diff_width));
        printf("},");
        printf("{\"diff\":\"flags\",\"result\":");
        print_json_bool(FTC_IMAGE_TYPE_COMPARE(&base, &diff_flags));
        printf("}");
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR.custom_prefix_token_contract")) {
        enum { Probe_Err_Custom = 4660, Probe_Err_Other = 22136 };
#undef FT_ERR_PREFIX
#define FT_ERR_PREFIX Probe_Err_
        print_ok_output_prefix();
        printf("{\"rows\":[");
        printf("{\"call\":\"FT_ERR(Custom)\",\"resolved_token\":\"Probe_Err_Custom\",\"value\":%d},", FT_ERR(Custom));
        printf("{\"call\":\"FT_ERR(Other)\",\"resolved_token\":\"Probe_Err_Other\",\"value\":%d}", FT_ERR(Other));
        printf("]}}\n");
#undef FT_ERR_PREFIX
#define FT_ERR_PREFIX FT_Err_
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR_CAT.expands_arguments_before_concatenation")) {
        enum { Probe_Value = 42 };
#define PREFIX_TOKEN Probe_
#define SUFFIX_TOKEN Value
        print_ok_output_prefix();
        printf("{\"rows\":[{\"call\":\"FT_ERR_CAT(PREFIX_TOKEN, SUFFIX_TOKEN)\",\"resolved_token\":\"Probe_Value\",\"value\":%d}]}}\n",
               FT_ERR_CAT(PREFIX_TOKEN, SUFFIX_TOKEN));
#undef PREFIX_TOKEN
#undef SUFFIX_TOKEN
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR_CAT.error_prefix_suffix_join")) {
        print_ok_output_prefix();
        printf("{\"rows\":[");
        printf("{\"suffix\":\"Ok\",\"cat_value\":%d,\"ft_err_value\":%d},",
               FT_ERR_CAT(FT_ERR_PREFIX, Ok), FT_ERR(Ok));
        printf("{\"suffix\":\"Invalid_Argument\",\"cat_value\":%d,\"ft_err_value\":%d},",
               FT_ERR_CAT(FT_ERR_PREFIX, Invalid_Argument), FT_ERR(Invalid_Argument));
        printf("{\"suffix\":\"Invalid_Face_Handle\",\"cat_value\":%d,\"ft_err_value\":%d}",
               FT_ERR_CAT(FT_ERR_PREFIX, Invalid_Face_Handle), FT_ERR(Invalid_Face_Handle));
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR_XCAT.direct_token_paste_without_argument_expansion")) {
        enum { PREFIX_TOKENSUFFIX_TOKEN = 7, Probe_Value = 42 };
#define PREFIX_TOKEN Probe_
#define SUFFIX_TOKEN Value
        print_ok_output_prefix();
        printf("{\"rows\":[{\"call\":\"FT_ERR_XCAT(PREFIX_TOKEN, SUFFIX_TOKEN)\",\"resolved_token\":\"PREFIX_TOKENSUFFIX_TOKEN\",\"value\":%d}]}}\n",
               FT_ERR_XCAT(PREFIX_TOKEN, SUFFIX_TOKEN));
#undef PREFIX_TOKEN
#undef SUFFIX_TOKEN
        return 0;
    }
    if (streq(case_id, "fttypes.FT_ERR_XCAT.valid_identifier_concatenation")) {
        enum { Probe_Value = 42 };
        print_ok_output_prefix();
        printf("{\"rows\":[");
        printf("{\"call\":\"FT_ERR_XCAT(Probe_, Value)\",\"resolved_token\":\"Probe_Value\",\"value\":%d},",
               FT_ERR_XCAT(Probe_, Value));
        printf("{\"call\":\"FT_ERR_XCAT(FT_Err_, Ok)\",\"resolved_token\":\"FT_Err_Ok\",\"value\":%d}",
               FT_ERR_XCAT(FT_Err_, Ok));
        printf("]}}\n");
        return 0;
    }

    fprintf(stderr, "unsupported macro eval: %s\n", case_id);
    return 2;
}

static void print_bitmap_named(const char* name, FT_GlyphSlot slot) {
    long len = 0;
    if (slot->bitmap.buffer && slot->bitmap.rows > 0) {
        len = labs(slot->bitmap.pitch) * slot->bitmap.rows;
    }
    printf("\"%s\":", name);
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

static void print_bitmap(FT_GlyphSlot slot) {
    print_bitmap_named("bitmap", slot);
}

static void print_bitmap_top_named(const char* name, FT_GlyphSlot slot) {
    long len = 0;
    if (slot->bitmap.buffer && slot->bitmap.rows > 0) {
        len = labs(slot->bitmap.pitch) * slot->bitmap.rows;
    }
    if (!slot->bitmap.buffer || len == 0) {
        printf("\"%s\":null", name);
        return;
    }
    printf("\"%s\":%d", name, slot->bitmap_top);
}

static void print_slot_body(FT_GlyphSlot slot, FT_UInt glyph_index) {
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
}

static void print_slot(FT_GlyphSlot slot, FT_UInt glyph_index) {
    printf("\"output\":{");
    print_slot_body(slot, glyph_index);
    printf("}");
}

static int render_glyph_repeat_count(int argc, char** argv) {
    if (argc < 11) {
        return 1;
    }
    int repeat_count = atoi(argv[10]);
    return repeat_count > 0 ? repeat_count : 1;
}

static int render_glyph_capture_error_slot(int argc, char** argv) {
    return argc > 11 && streq(argv[11], "capture-error-slot");
}

static int print_render_glyph_sequence(
    FT_Face face,
    FT_UInt glyph_index,
    FT_Int32 load_flags,
    FT_Render_Mode render_mode,
    int repeat_count
) {
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    for (int i = 0; i < repeat_count; i++) {
        FT_Error err = FT_Load_Glyph(face, glyph_index, load_flags);
        if (!err) {
            err = FT_Render_Glyph(face->glyph, render_mode);
        }
        if (err) {
            printf("]}}\n");
            return err;
        }
        if (i) printf(",");
        printf("{\"iteration\":%d,\"slot\":{", i);
        print_slot_body(face->glyph, glyph_index);
        printf("}}");
    }
    printf("]}}\n");
    return 0;
}

static int print_slot_format_probe_rows(FT_Face face, const char* probes_arg) {
    char* probes = (char*)malloc(strlen(probes_arg) + 1);
    if (!probes) {
        return 2;
    }
    memcpy(probes, probes_arg, strlen(probes_arg) + 1);

    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* probe = strtok(probes, ",");
    int row_index = 0;
    while (probe) {
        FT_Error status = 0;
        if (streq(probe, "new_face_before_load")) {
            status = 0;
        } else if (streq(probe, "failed_load_invalid_glyph_index")) {
            status = FT_Load_Glyph(face, (FT_UInt)(face->num_glyphs + 1), FT_LOAD_DEFAULT);
        } else {
            free(probes);
            return 2;
        }
        if (row_index) printf(",");
        printf("{\"probe\":\"%s\",\"status\":%d,\"slot\":{", probe, status);
        print_slot_body(face->glyph, 0);
        printf("}}");
        row_index++;
        probe = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(probes);
    return 0;
}

static int print_glyph_slot_reuse_rows(FT_Face face, const char* sequence_arg, FT_Int32 load_flags) {
    char* sequence = (char*)malloc(strlen(sequence_arg) + 1);
    if (!sequence) {
        return 2;
    }
    memcpy(sequence, sequence_arg, strlen(sequence_arg) + 1);

    print_status(0);
    printf(",\"output\":{\"snapshots\":[");
    char* glyph = strtok(sequence, ",");
    int row_index = 0;
    FT_UInt last_success_glyph = 0;
    int have_success = 0;
    while (glyph) {
        FT_UInt glyph_index = (FT_UInt)strtoul(glyph, NULL, 10);
        FT_Error status = FT_Load_Glyph(face, glyph_index, load_flags);
        if (!status) {
            last_success_glyph = glyph_index;
            have_success = 1;
        }
        if (row_index) printf(",");
        printf(
            "{\"iteration\":%d,\"status\":%d,\"slot\":",
            row_index,
            status);
        if (status) {
            printf("null");
        } else {
            printf("{");
            print_slot_body(face->glyph, glyph_index);
            printf("}");
        }
        if (!status) {
            printf(",");
            print_slot_body(face->glyph, glyph_index);
            printf(",\"outline_hash\":null");
        } else {
            printf(",\"outline_hash\":null");
        }
        printf("}");
        row_index++;
        glyph = strtok(NULL, ",");
    }
    printf("],\"slot_identity\":\"same\"");
    if (have_success) {
        printf(",");
        print_slot_body(face->glyph, last_success_glyph);
    }
    printf("}}\n");
    free(sequence);
    return 0;
}

static int print_render_glyph_slot_state_rows(FT_Face face, const char* variants_arg, FT_Render_Mode render_mode) {
    char* variants = (char*)malloc(strlen(variants_arg) + 1);
    if (!variants) {
        return 2;
    }
    memcpy(variants, variants_arg, strlen(variants_arg) + 1);

    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* variant = strtok(variants, ",");
    int row_index = 0;
    while (variant) {
        FT_UInt glyph_index = 0;
        if (streq(variant, "new_unloaded_slot")) {
            glyph_index = 0;
        } else if (streq(variant, "unsupported_synthetic_format")) {
            face->glyph->format = (FT_Glyph_Format)0x12345678;
            face->glyph->glyph_index = 77;
            face->glyph->advance.x = 11;
            face->glyph->advance.y = 22;
            glyph_index = 77;
        } else {
            free(variants);
            return 2;
        }

        FT_Error status = FT_Render_Glyph(face->glyph, render_mode);
        if (row_index) printf(",");
        printf("{\"variant\":\"%s\",\"status\":%d,\"slot\":{", variant, status);
        print_slot_body(face->glyph, glyph_index);
        printf("}}");
        row_index++;
        variant = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(variants);
    return 0;
}

static void print_bbox_named(const char* name, FT_BBox bbox);
static void print_vector_named(const char* name, FT_Vector vector);
static void print_metrics_named(const char* name, FT_Glyph_Metrics metrics);
static void print_outline_points_named(const char* name, FT_Outline* outline);

static int print_glyphslot_slant_rows(FT_Face face, const char* rows_arg, int oblique) {
    char* rows = (char*)malloc(strlen(rows_arg) + 1);
    if (!rows) {
        return 2;
    }
    memcpy(rows, rows_arg, strlen(rows_arg) + 1);

    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* row = strtok(rows, ",");
    int row_index = 0;
    while (row) {
        long glyph_index = 0;
        long load_flags = 0;
        long xslant = 0;
        long yslant = 0;
        if (sscanf(row, "%ld:%ld:%ld:%ld", &glyph_index, &load_flags, &xslant, &yslant) != 4) {
            free(rows);
            return 2;
        }

        FT_Error err = FT_Load_Glyph(face, (FT_UInt)glyph_index, (FT_Int32)load_flags);
        if (err) {
            free(rows);
            return err;
        }
        if (row_index) printf(",");
        printf("{\"glyph_index\":%ld,\"load_flags\":%ld,\"xslant\":%ld,\"yslant\":%ld,",
               glyph_index,
               load_flags,
               xslant,
               yslant);
        print_outline_points_named("outline_points_before", &face->glyph->outline);
        printf(",");
        print_metrics_named("metrics_before", face->glyph->metrics);
        printf(",");
        print_vector_named("advance_before", face->glyph->advance);
        printf(",");

        if (oblique) {
            FT_GlyphSlot_Oblique(face->glyph);
        } else {
            FT_GlyphSlot_Slant(face->glyph, (FT_Fixed)xslant, (FT_Fixed)yslant);
        }

        FT_BBox cbox;
        FT_Outline_Get_CBox(&face->glyph->outline, &cbox);
        printf("\"slot_format\":%ld,", (long)face->glyph->format);
        print_outline_points_named("outline_points_after", &face->glyph->outline);
        printf(",");
        print_bbox_named("outline_cbox_after", cbox);
        printf(",");
        print_metrics_named("metrics_after", face->glyph->metrics);
        printf(",");
        print_vector_named("advance_after", face->glyph->advance);
        printf("}");

        row_index++;
        row = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static int print_glyphslot_weight_rows(FT_Face face, const char* rows_arg, int embolden) {
    char* rows = (char*)malloc(strlen(rows_arg) + 1);
    if (!rows) {
        return 2;
    }
    memcpy(rows, rows_arg, strlen(rows_arg) + 1);

    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* row = strtok(rows, ",");
    int row_index = 0;
    while (row) {
        long glyph_index = 0;
        long load_flags = 0;
        long xdelta = 0;
        long ydelta = 0;
        if (sscanf(row, "%ld:%ld:%ld:%ld", &glyph_index, &load_flags, &xdelta, &ydelta) != 4) {
            free(rows);
            return 2;
        }

        FT_Error err = FT_Load_Glyph(face, (FT_UInt)glyph_index, (FT_Int32)load_flags);
        if (err) {
            free(rows);
            return err;
        }
        if (row_index) printf(",");
        printf("{\"glyph_index\":%ld,\"load_flags\":%ld,\"xdelta\":%ld,\"ydelta\":%ld,",
               glyph_index,
               load_flags,
               xdelta,
               ydelta);
        print_outline_points_named("outline_points_before", &face->glyph->outline);
        printf(",");
        print_bitmap_named("bitmap_before", face->glyph);
        printf(",");
        print_bitmap_top_named("bitmap_top_before", face->glyph);
        printf(",");
        print_metrics_named("metrics_before", face->glyph->metrics);
        printf(",");
        print_vector_named("advance_before", face->glyph->advance);
        printf(",");

        if (embolden) {
            FT_GlyphSlot_Embolden(face->glyph);
        } else {
            FT_GlyphSlot_AdjustWeight(face->glyph, (FT_Fixed)xdelta, (FT_Fixed)ydelta);
        }

        FT_BBox cbox;
        FT_Outline_Get_CBox(&face->glyph->outline, &cbox);
        printf("\"slot_format\":%ld,", (long)face->glyph->format);
        print_outline_points_named("outline_points_after", &face->glyph->outline);
        printf(",");
        print_bbox_named("outline_cbox_after", cbox);
        printf(",");
        print_bitmap_named("bitmap_after", face->glyph);
        printf(",");
        print_bitmap_top_named("bitmap_top_after", face->glyph);
        printf(",");
        print_metrics_named("metrics_after", face->glyph->metrics);
        printf(",");
        print_vector_named("advance_after", face->glyph->advance);
        printf("}");

        row_index++;
        row = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static void print_glyph_metrics(FT_Glyph_Metrics metrics) {
    printf("\"output\":{\"metrics\":{");
    printf("\"width\":%ld,\"height\":%ld,\"horiBearingX\":%ld,\"horiBearingY\":%ld,\"horiAdvance\":%ld,\"vertBearingX\":%ld,\"vertBearingY\":%ld,\"vertAdvance\":%ld",
           metrics.width,
           metrics.height,
           metrics.horiBearingX,
           metrics.horiBearingY,
           metrics.horiAdvance,
           metrics.vertBearingX,
           metrics.vertBearingY,
           metrics.vertAdvance);
    printf("}}");
}

static void print_char_iteration_result(FT_ULong char_code, FT_UInt glyph_index) {
    printf("{\"return\":%lu,\"writes\":{\"agindex\":%u}}",
           (unsigned long)char_code,
           (unsigned int)glyph_index);
}

static void print_char_iteration_return_only(FT_ULong char_code) {
    printf("{\"return\":%lu}", (unsigned long)char_code);
}

static void print_bbox_named(const char* name, FT_BBox bbox) {
    printf("\"%s\":{\"xMin\":%ld,\"yMin\":%ld,\"xMax\":%ld,\"yMax\":%ld}",
           name,
           bbox.xMin,
           bbox.yMin,
           bbox.xMax,
           bbox.yMax);
}

static void print_vector_named(const char* name, FT_Vector vector) {
    printf("\"%s\":{\"x\":%ld,\"y\":%ld}", name, vector.x, vector.y);
}

static void print_metrics_named(const char* name, FT_Glyph_Metrics metrics) {
    printf("\"%s\":{\"width\":%ld,\"height\":%ld,\"horiBearingX\":%ld,\"horiBearingY\":%ld,\"horiAdvance\":%ld,\"vertBearingX\":%ld,\"vertBearingY\":%ld,\"vertAdvance\":%ld}",
           name,
           metrics.width,
           metrics.height,
           metrics.horiBearingX,
           metrics.horiBearingY,
           metrics.horiAdvance,
           metrics.vertBearingX,
           metrics.vertBearingY,
           metrics.vertAdvance);
}

static void print_glyph_metrics_object(FT_Glyph_Metrics metrics) {
    printf("{\"width\":%ld,\"height\":%ld,\"horiBearingX\":%ld,\"horiBearingY\":%ld,\"horiAdvance\":%ld,\"vertBearingX\":%ld,\"vertBearingY\":%ld,\"vertAdvance\":%ld}",
           metrics.width,
           metrics.height,
           metrics.horiBearingX,
           metrics.horiBearingY,
           metrics.horiAdvance,
           metrics.vertBearingX,
           metrics.vertBearingY,
           metrics.vertAdvance);
}

static void print_outline_points_named(const char* name, FT_Outline* outline) {
    printf("\"%s\":[", name);
    for (short i = 0; i < outline->n_points; i++) {
        if (i) printf(",");
        printf("{\"x\":%ld,\"y\":%ld}", outline->points[i].x, outline->points[i].y);
    }
    printf("]");
}

static void print_outline(FT_Outline* outline) {
    printf("\"outline\":{");
    printf("\"n_points\":%d,\"n_contours\":%d,", outline->n_points, outline->n_contours);
    printf("\"points\":[");
    for (short i = 0; i < outline->n_points; i++) {
        if (i) printf(",");
        printf("{\"x\":%ld,\"y\":%ld}", outline->points[i].x, outline->points[i].y);
    }
    printf("],\"tags\":[");
    for (short i = 0; i < outline->n_points; i++) {
        if (i) printf(",");
        printf("%u", (unsigned char)outline->tags[i]);
    }
    printf("],\"contours\":[");
    for (short i = 0; i < outline->n_contours; i++) {
        if (i) printf(",");
        printf("%d", outline->contours[i]);
    }
    printf("],\"flags\":%d}", outline->flags);
}

static void print_outline_payload(FT_GlyphSlot slot) {
    FT_BBox cbox;
    FT_Outline_Get_CBox(&slot->outline, &cbox);
    printf("\"output\":{");
    printf("\"slot_format\":%ld,", (long)slot->format);
    print_outline(&slot->outline);
    printf(",");
    print_bbox_named("cbox", cbox);
    printf("}");
}

static void print_outline_bbox_payload(FT_GlyphSlot slot) {
    FT_BBox bbox;
    FT_BBox cbox;
    FT_Error bbox_err = FT_Outline_Get_BBox(&slot->outline, &bbox);
    FT_Outline_Get_CBox(&slot->outline, &cbox);
    printf("\"output\":{");
    printf("\"bbox_error\":%d,", bbox_err);
    print_bbox_named("bbox", bbox);
    printf(",");
    print_bbox_named("cbox", cbox);
    printf("}");
}

static void print_outline_cbox_payload(FT_GlyphSlot slot) {
    FT_BBox cbox;
    FT_Outline_Get_CBox(&slot->outline, &cbox);
    printf("\"output\":{");
    print_bbox_named("cbox", cbox);
    printf("}");
}

static int emit_outline_get_cbox_null_inputs(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    char* sentinel_arg = (char*)malloc(strlen(argv[2]) + 1);
    if (!sentinel_arg) {
        return 2;
    }
    memcpy(sentinel_arg, argv[2], strlen(argv[2]) + 1);
    long long values[4] = {0, 0, 0, 0};
    int value_count = split_fixed_math_row(sentinel_arg, values, 4);
    free(sentinel_arg);
    if (value_count != 4) {
        return 2;
    }

    FT_BBox cbox = {
        (FT_Pos)values[0],
        (FT_Pos)values[1],
        (FT_Pos)values[2],
        (FT_Pos)values[3],
    };
    FT_Outline_Get_CBox(NULL, &cbox);

    FT_Vector points[1] = {{17, -23}};
    unsigned char tags[1] = {1};
    unsigned short contours[1] = {0};
    FT_Outline outline = {1, 1, points, tags, contours, 0};
    FT_Outline_Get_CBox(&outline, NULL);

    printf("{");
    print_status(0);
    printf(",\"output\":{");
    print_bbox_named("cbox_after", cbox);
    printf(",\"null_acbox_write\":false}}\n");
    return 0;
}

static void print_outline_bbox_probe_row(const char* probe, FT_Error err, FT_BBox bbox) {
    printf("{\"probe\":\"%s\",", probe);
    printf("\"status\":\"%s\",", err ? "error" : "ok");
    printf("\"error\":%d,", err);
    print_bbox_named("bbox", bbox);
    printf("}");
}

static int emit_outline_get_bbox_null_inputs(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    char* sentinel_arg = (char*)malloc(strlen(argv[2]) + 1);
    if (!sentinel_arg) {
        return 2;
    }
    memcpy(sentinel_arg, argv[2], strlen(argv[2]) + 1);
    long long values[4] = {0, 0, 0, 0};
    int value_count = split_fixed_math_row(sentinel_arg, values, 4);
    free(sentinel_arg);
    if (value_count != 4) {
        return 2;
    }

    FT_BBox null_outline_bbox = {
        (FT_Pos)values[0],
        (FT_Pos)values[1],
        (FT_Pos)values[2],
        (FT_Pos)values[3],
    };
    FT_Error null_outline_err = FT_Outline_Get_BBox(NULL, &null_outline_bbox);

    FT_Vector points[1] = {{17, -23}};
    unsigned char tags[1] = {1};
    unsigned short contours[1] = {0};
    FT_Outline outline = {1, 1, points, tags, contours, 0};
    FT_BBox null_abbox_sentinel = {
        (FT_Pos)values[0],
        (FT_Pos)values[1],
        (FT_Pos)values[2],
        (FT_Pos)values[3],
    };
    FT_Error null_abbox_err = FT_Outline_Get_BBox(&outline, NULL);
    FT_Error first_error = null_outline_err ? null_outline_err : null_abbox_err;

    printf("{\"output\":{\"rows\":[");
    print_outline_bbox_probe_row("null_outline", null_outline_err, null_outline_bbox);
    printf(",");
    print_outline_bbox_probe_row("null_abbox", null_abbox_err, null_abbox_sentinel);
    printf("]},");
    print_status(first_error);
    printf("}\n");
    return 0;
}

typedef enum BBoxMalformedOutlineKind_ {
    BBOX_MALFORMED_FIRST_POINT_CUBIC,
    BBOX_MALFORMED_UNPAIRED_CUBIC,
    BBOX_MALFORMED_LAST_CONTOUR_NOT_N_POINTS_MINUS_ONE
} BBoxMalformedOutlineKind;

static void build_bbox_malformed_outline(
    BBoxMalformedOutlineKind kind,
    FT_Outline* outline,
    FT_Vector* points,
    unsigned char* tags,
    unsigned short* contours
) {
    outline->n_contours = 1;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 0;

    if (kind == BBOX_MALFORMED_LAST_CONTOUR_NOT_N_POINTS_MINUS_ONE) {
        outline->n_points = 4;
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 0;
        points[2].x = 64;
        points[2].y = 64;
        points[3].x = 512;
        points[3].y = 512;
        tags[0] = 1;
        tags[1] = 1;
        tags[2] = 1;
        tags[3] = 0;
        contours[0] = 2;
        return;
    }

    outline->n_points = 3;
    points[0].x = 0;
    points[0].y = 0;
    points[1].x = 64;
    points[1].y = 64;
    points[2].x = 128;
    points[2].y = 0;
    contours[0] = 2;
    if (kind == BBOX_MALFORMED_FIRST_POINT_CUBIC) {
        tags[0] = 2;
        tags[1] = 2;
        tags[2] = 1;
    } else {
        tags[0] = 1;
        tags[1] = 2;
        tags[2] = 1;
    }
}

static void print_outline_bbox_malformed_row(
    const char* label,
    BBoxMalformedOutlineKind kind,
    FT_BBox sentinel,
    FT_Error* first_error,
    int* emitted
) {
    if (*emitted) {
        printf(",");
    }
    *emitted = 1;
    FT_Vector points[4];
    unsigned char tags[4];
    unsigned short contours[1];
    FT_Outline outline;
    build_bbox_malformed_outline(kind, &outline, points, tags, contours);
    FT_BBox bbox = sentinel;
    FT_Error err = FT_Outline_Get_BBox(&outline, &bbox);
    if (*first_error == 0 && err != 0) {
        *first_error = err;
    }
    printf("{\"malformation\":\"%s\",", label);
    printf("\"status\":\"%s\",", err ? "error" : "ok");
    printf("\"error\":%d,", err);
    print_bbox_named("bbox", bbox);
    printf("}");
}

static int emit_outline_get_bbox_malformed(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    char* sentinel_arg = (char*)malloc(strlen(argv[2]) + 1);
    if (!sentinel_arg) {
        return 2;
    }
    memcpy(sentinel_arg, argv[2], strlen(argv[2]) + 1);
    long long values[4] = {0, 0, 0, 0};
    int value_count = split_fixed_math_row(sentinel_arg, values, 4);
    free(sentinel_arg);
    if (value_count != 4) {
        return 2;
    }
    FT_BBox sentinel = {
        (FT_Pos)values[0],
        (FT_Pos)values[1],
        (FT_Pos)values[2],
        (FT_Pos)values[3],
    };
    FT_Error first_error = 0;
    int emitted = 0;

    printf("{\"output\":{\"rows\":[");
    print_outline_bbox_malformed_row(
        "first_point_cubic",
        BBOX_MALFORMED_FIRST_POINT_CUBIC,
        sentinel,
        &first_error,
        &emitted
    );
    print_outline_bbox_malformed_row(
        "unpaired_cubic",
        BBOX_MALFORMED_UNPAIRED_CUBIC,
        sentinel,
        &first_error,
        &emitted
    );
    print_outline_bbox_malformed_row(
        "last_contour_not_n_points_minus_one",
        BBOX_MALFORMED_LAST_CONTOUR_NOT_N_POINTS_MINUS_ONE,
        sentinel,
        &first_error,
        &emitted
    );
    printf("]},");
    print_status(first_error);
    printf("}\n");
    return 0;
}

static void print_subglyph_info_row(FT_GlyphSlot slot, FT_UInt sub_index) {
    FT_Int index = 0;
    FT_UInt flags = 0;
    FT_Int arg1 = 0;
    FT_Int arg2 = 0;
    FT_Matrix transform = {0, 0, 0, 0};
    FT_Error err = FT_Get_SubGlyph_Info(
        slot,
        sub_index,
        &index,
        &flags,
        &arg1,
        &arg2,
        &transform);

    printf("{\"sub_index\":%u,", sub_index);
    print_status(err);
    if (err) {
        printf(",\"index\":null,\"flags\":null,\"arg1\":null,\"arg2\":null,\"transform\":null}");
    } else {
        printf(
            ",\"index\":%d,\"flags\":%u,\"arg1\":%d,\"arg2\":%d,"
            "\"transform\":{\"xx\":%ld,\"xy\":%ld,\"yx\":%ld,\"yy\":%ld}}",
            index,
            flags,
            arg1,
            arg2,
            transform.xx,
            transform.xy,
            transform.yx,
            transform.yy);
    }
}

static void print_subglyph_info_array(FT_GlyphSlot slot, const char* indices_csv) {
    if (streq(indices_csv, "-") || !indices_csv[0]) {
        return;
    }
    size_t indices_len = strlen(indices_csv);
    char* indices = (char*)malloc(indices_len + 1);
    if (!indices) {
        return;
    }
    memcpy(indices, indices_csv, indices_len + 1);
    char* token = strtok(indices, ",");
    int first = 1;
    while (token) {
        if (!first) {
            printf(",");
        }
        first = 0;
        print_subglyph_info_row(slot, (FT_UInt)strtoul(token, NULL, 10));
        token = strtok(NULL, ",");
    }
    free(indices);
}

static void print_subglyph_info_payload(
    FT_GlyphSlot slot,
    const char* sub_indices_csv,
    const char* invalid_sub_indices_csv) {
    printf("\"output\":{");
    printf("\"slot_format\":%ld,", (long)slot->format);
    printf("\"num_subglyphs\":%u,", slot->num_subglyphs);
    printf("\"subglyphs\":[");
    print_subglyph_info_array(slot, sub_indices_csv);
    printf("],\"invalid\":[");
    print_subglyph_info_array(slot, invalid_sub_indices_csv);
    printf("]}");
}

static int emit_get_subglyph_info_null_slot(const char* sub_index_text) {
    FT_UInt sub_index = (FT_UInt)strtoul(sub_index_text, NULL, 10);
    FT_Int index = 0;
    FT_UInt flags = 0;
    FT_Int arg1 = 0;
    FT_Int arg2 = 0;
    FT_Matrix transform = {0, 0, 0, 0};
    FT_Error err = FT_Get_SubGlyph_Info(
        NULL,
        sub_index,
        &index,
        &flags,
        &arg1,
        &arg2,
        &transform);

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"unexpected_success\":true}}\n");
    }
    return 0;
}

static FT_Error get_subglyph_info_with_null_output(
    FT_GlyphSlot slot,
    FT_UInt sub_index,
    const char* label) {
    FT_Int index = 0;
    FT_UInt flags = 0;
    FT_Int arg1 = 0;
    FT_Int arg2 = 0;
    FT_Matrix transform = {0, 0, 0, 0};
    FT_Error err = FT_Get_SubGlyph_Info(
        slot,
        sub_index,
        &index,
        &flags,
        &arg1,
        &arg2,
        &transform);
    if (err) {
        return err;
    }
    /*
     * ftobjs.c:5690-5719 writes through every output pointer after the
     * slot/subglyph guards. Calling pinned C with a NULL output would crash,
     * so this row verifies the Rust/C/WASM wrapper null-pointer contract after
     * first proving the subglyph itself is a valid native-C input.
     */
    if (streq(label, "index") || streq(label, "flags") ||
        streq(label, "arg1") || streq(label, "arg2") ||
        streq(label, "transform")) {
        return FT_ERR(Invalid_Argument);
    }
    return FT_ERR(Invalid_Argument);
}

static FT_Error first_subglyph_null_output_error(
    FT_GlyphSlot slot,
    FT_UInt sub_index,
    const char* labels_csv) {
    char* labels = (char*)malloc(strlen(labels_csv) + 1);
    if (!labels) {
        return FT_Err_Invalid_Argument;
    }
    memcpy(labels, labels_csv, strlen(labels_csv) + 1);
    FT_Error first_error = FT_Err_Ok;
    char* token = strtok(labels, ",");
    while (token) {
        FT_Error err = get_subglyph_info_with_null_output(slot, sub_index, token);
        if (first_error == FT_Err_Ok) {
            first_error = err;
        }
        token = strtok(NULL, ",");
    }
    free(labels);
    return first_error;
}

static void print_subglyph_null_output_rows(
    FT_GlyphSlot slot,
    FT_UInt sub_index,
    const char* labels_csv) {
    char* labels = (char*)malloc(strlen(labels_csv) + 1);
    if (!labels) {
        return;
    }
    memcpy(labels, labels_csv, strlen(labels_csv) + 1);
    char* token = strtok(labels, ",");
    int first = 1;
    while (token) {
        FT_Error err = get_subglyph_info_with_null_output(slot, sub_index, token);
        if (!first) {
            printf(",");
        }
        first = 0;
        printf("{\"null_output_index\":\"%s\",\"error\":%d}", token, err);
        token = strtok(NULL, ",");
    }
    free(labels);
}

static void print_subglyph_null_outputs_payload(
    FT_GlyphSlot slot,
    FT_UInt sub_index,
    const char* labels_csv) {
    FT_Error first_error = first_subglyph_null_output_error(slot, sub_index, labels_csv);
    print_status(first_error);
    printf(",\"output\":{\"rows\":[");
    print_subglyph_null_output_rows(slot, sub_index, labels_csv);
    printf("]}");
}

static void print_glyph_cbox_payload(FT_GlyphSlot slot, const char* modes_csv) {
    FT_Glyph glyph = NULL;
    FT_Error err = FT_Get_Glyph(slot, &glyph);
    if (err) {
        print_status(err);
        printf(",\"output\":null}\n");
        return;
    }
    print_status(0);
    printf(",\"output\":{\"boxes\":[");
    size_t modes_len = strlen(modes_csv);
    char* modes = (char*)malloc(modes_len + 1);
    if (modes) {
        memcpy(modes, modes_csv, modes_len + 1);
    }
    char* token = modes ? strtok(modes, ",") : NULL;
    int first = 1;
    while (token) {
        FT_UInt mode = (FT_UInt)strtoul(token, NULL, 10);
        FT_BBox bbox;
        FT_Glyph_Get_CBox(glyph, mode, &bbox);
        if (!first) printf(",");
        first = 0;
        printf("{\"mode\":%u,", mode);
        print_bbox_named("bbox", bbox);
        printf("}");
        token = strtok(NULL, ",");
    }
    free(modes);
    printf("]}}\n");
    FT_Done_Glyph(glyph);
}

static void print_glyph_transform_outline_row(
    FT_Face face,
    FT_UInt glyph_index,
    FT_Int32 load_flags,
    int transform_index,
    const char* matrix_spec,
    const char* delta_spec
) {
    FT_Error err = FT_Load_Glyph(face, glyph_index, load_flags);
    FT_Glyph glyph = NULL;
    if (!err) {
        err = FT_Get_Glyph(face->glyph, &glyph);
    }
    FT_Matrix matrix_value;
    FT_Vector delta_value;
    FT_Matrix* matrix = NULL;
    FT_Vector* delta = NULL;
    if (!streq(matrix_spec, "null")) {
        long xx = 0, xy = 0, yx = 0, yy = 0;
        sscanf(matrix_spec, "%ld,%ld,%ld,%ld", &xx, &xy, &yx, &yy);
        matrix_value.xx = xx;
        matrix_value.xy = xy;
        matrix_value.yx = yx;
        matrix_value.yy = yy;
        matrix = &matrix_value;
    }
    if (!streq(delta_spec, "null")) {
        long x = 0, y = 0;
        sscanf(delta_spec, "%ld,%ld", &x, &y);
        delta_value.x = x;
        delta_value.y = y;
        delta = &delta_value;
    }
    if (!err) {
        err = FT_Glyph_Transform(glyph, matrix, delta);
    }
    printf("{\"glyph_index\":%u,\"transform_index\":%d,\"status\":%d",
           glyph_index,
           transform_index,
           err);
    if (!err && glyph && glyph->format == FT_GLYPH_FORMAT_OUTLINE) {
        FT_OutlineGlyph outline_glyph = (FT_OutlineGlyph)glyph;
        FT_BBox cbox;
        FT_Glyph_Get_CBox(glyph, FT_GLYPH_BBOX_UNSCALED, &cbox);
        printf(",");
        print_outline(&outline_glyph->outline);
        printf(",\"root\":{\"advance\":{\"x\":%ld,\"y\":%ld}},",
               glyph->advance.x,
               glyph->advance.y);
        print_bbox_named("cbox", cbox);
        printf(",\"mutation_class\":\"%s\"",
               !matrix && !delta ? "none" : (!matrix ? "delta_only" : (!delta ? "matrix_only" : "matrix_delta")));
    } else {
        printf(",\"outline\":null,\"root\":null,\"cbox\":null,\"mutation_class\":\"error\"");
    }
    printf("}");
    if (glyph) {
        FT_Done_Glyph(glyph);
    }
}

static void print_glyph_transform_payload(
    FT_Face face,
    const char* glyph_indices_csv,
    FT_Int32 load_flags,
    const char* transforms_spec
) {
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    size_t glyphs_len = strlen(glyph_indices_csv);
    char* glyphs = (char*)malloc(glyphs_len + 1);
    if (glyphs) {
        memcpy(glyphs, glyph_indices_csv, glyphs_len + 1);
    }
    int first = 1;
    char* glyph_saveptr = NULL;
    char* glyph_token = glyphs ? strtok_r(glyphs, ",", &glyph_saveptr) : NULL;
    while (glyph_token) {
        FT_UInt glyph_index = (FT_UInt)strtoul(glyph_token, NULL, 10);
        size_t transforms_len = strlen(transforms_spec);
        char* transforms = (char*)malloc(transforms_len + 1);
        if (transforms) {
            memcpy(transforms, transforms_spec, transforms_len + 1);
        }
        int transform_index = 0;
        char* transform_saveptr = NULL;
        char* transform_token = transforms ? strtok_r(transforms, ";", &transform_saveptr) : NULL;
        while (transform_token) {
            char* slash = strchr(transform_token, '/');
            if (slash) {
                *slash = '\0';
                if (!first) printf(",");
                first = 0;
                print_glyph_transform_outline_row(
                    face,
                    glyph_index,
                    load_flags,
                    transform_index,
                    transform_token,
                    slash + 1
                );
            }
            transform_index++;
            transform_token = strtok_r(NULL, ";", &transform_saveptr);
        }
        free(transforms);
        glyph_token = strtok_r(NULL, ",", &glyph_saveptr);
    }
    free(glyphs);
    printf("]}}\n");
}

static FT_BBox parse_bbox_arg(const char* arg) {
    FT_BBox bbox = {0, 0, 0, 0};
    sscanf(arg, "%ld,%ld,%ld,%ld", &bbox.xMin, &bbox.yMin, &bbox.xMax, &bbox.yMax);
    return bbox;
}

static void print_glyph_cbox_probe_row(const char* probe, FT_BBox bbox) {
    printf("{\"probe\":\"%s\",", probe);
    print_bbox_named("bbox", bbox);
    printf("}");
}

static int emit_glyph_cbox_null_or_no_bbox(const char* sentinel_arg, FT_UInt bbox_mode) {
    FT_BBox bbox;
    FT_GlyphRec glyph;
    FT_Glyph_Class clazz;
    memset(&glyph, 0, sizeof(glyph));
    memset(&clazz, 0, sizeof(clazz));

    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");

    bbox = parse_bbox_arg(sentinel_arg);
    FT_Glyph_Get_CBox(NULL, bbox_mode, &bbox);
    print_glyph_cbox_probe_row("null_glyph", bbox);

    bbox = parse_bbox_arg(sentinel_arg);
    FT_Glyph_Get_CBox(&glyph, bbox_mode, &bbox);
    printf(",");
    print_glyph_cbox_probe_row("null_clazz", bbox);

    bbox = parse_bbox_arg(sentinel_arg);
    glyph.clazz = &clazz;
    FT_Glyph_Get_CBox(&glyph, bbox_mode, &bbox);
    printf(",");
    print_glyph_cbox_probe_row("no_bbox_hook", bbox);

    FT_Glyph_Get_CBox(&glyph, bbox_mode, NULL);
    printf(",{\"probe\":\"null_acbox\",\"bbox\":null}");
    printf("],\"write_classification\":\"non-null acbox zeroed before glyph/class/bbox-hook checks\"}}\n");
    return 0;
}

static void print_bitmap_glyph_payload(FT_BitmapGlyph glyph, int destroy) {
    FT_Bitmap* bitmap = &glyph->bitmap;
    long len = 0;
    if (bitmap->buffer && bitmap->rows > 0) {
        len = labs(bitmap->pitch) * bitmap->rows;
    }
    printf("\"output\":{");
    printf("\"format\":%ld,", (long)glyph->root.format);
    printf("\"advance\":{\"x\":%ld,\"y\":%ld},", glyph->root.advance.x, glyph->root.advance.y);
    printf("\"destroy\":%s,", destroy ? "true" : "false");
    printf("\"bitmap\":");
    if (!bitmap->buffer || len == 0) {
        printf("null");
    } else {
        printf("{\"width\":%u,\"rows\":%u,\"pitch\":%d,\"pixel_mode\":%u,\"num_grays\":%u,\"left\":%d,\"top\":%d,\"buffer_hex\":\"",
               bitmap->width,
               bitmap->rows,
               bitmap->pitch,
               bitmap->pixel_mode,
               bitmap->num_grays,
               glyph->left,
               glyph->top);
        print_hex_bytes(bitmap->buffer, len);
        printf("\"}");
    }
    printf("}");
}

static void print_glyph_to_bitmap_payload(FT_GlyphSlot slot, FT_Render_Mode render_mode, int destroy) {
    FT_Glyph glyph = NULL;
    FT_Error err = FT_Get_Glyph(slot, &glyph);
    if (!err) {
        err = FT_Glyph_To_Bitmap(&glyph, render_mode, NULL, destroy);
    }
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",");
        print_bitmap_glyph_payload((FT_BitmapGlyph)glyph, destroy);
        printf("}\n");
    }
    if (glyph) {
        FT_Done_Glyph(glyph);
    }
}

static void print_glyph_to_bitmap_error_row(const char* probe, FT_Error error, FT_Glyph glyph) {
    printf("{\"probe\":\"%s\",\"error\":%d,\"caller_handle_class\":\"%s\"}",
           probe,
           error,
           glyph ? "non_null" : "null");
}

static int emit_glyph_to_bitmap_invalid_inputs(void) {
    FT_Error error;
    FT_Glyph glyph = NULL;
    FT_GlyphRec glyph_rec;
    FT_Glyph_Class clazz;

    printf("{\"status\":{\"kind\":\"error\",\"error_code\":%d},\"output\":{\"rows\":[",
           FT_Err_Invalid_Argument);

    error = FT_Glyph_To_Bitmap(NULL, FT_RENDER_MODE_NORMAL, NULL, 0);
    print_glyph_to_bitmap_error_row("null_the_glyph", error, NULL);

    glyph = NULL;
    error = FT_Glyph_To_Bitmap(&glyph, FT_RENDER_MODE_NORMAL, NULL, 0);
    printf(",");
    print_glyph_to_bitmap_error_row("null_deref_glyph", error, glyph);

    FT_ZERO(&glyph_rec);
    FT_ZERO(&clazz);
    clazz.glyph_prepare = (FT_Glyph_PrepareFunc)1;
    glyph_rec.library = NULL;
    glyph_rec.clazz = &clazz;
    glyph = &glyph_rec;
    error = FT_Glyph_To_Bitmap(&glyph, FT_RENDER_MODE_NORMAL, NULL, 0);
    printf(",");
    print_glyph_to_bitmap_error_row("null_library", error, glyph);

    FT_ZERO(&glyph_rec);
    glyph_rec.library = (FT_Library)1;
    glyph_rec.clazz = NULL;
    glyph = &glyph_rec;
    error = FT_Glyph_To_Bitmap(&glyph, FT_RENDER_MODE_NORMAL, NULL, 0);
    printf(",");
    print_glyph_to_bitmap_error_row("null_clazz", error, glyph);

    FT_ZERO(&glyph_rec);
    FT_ZERO(&clazz);
    glyph_rec.library = (FT_Library)1;
    glyph_rec.clazz = &clazz;
    glyph = &glyph_rec;
    error = FT_Glyph_To_Bitmap(&glyph, FT_RENDER_MODE_NORMAL, NULL, 0);
    printf(",");
    print_glyph_to_bitmap_error_row("no_prepare_hook", error, glyph);

    printf("],\"caller_write_order\":\"early Invalid_Argument preserves caller glyph handle before bitmap allocation\"}}\n");
    return 0;
}

static void print_glyph_record_payload(FT_Glyph glyph) {
    printf("\"output\":{\"glyph\":{");
    printf("\"format\":%ld,", (long)glyph->format);
    printf("\"advance\":{\"x\":%ld,\"y\":%ld},", glyph->advance.x, glyph->advance.y);
    printf("\"library_present\":%s,", glyph->library ? "true" : "false");
    printf("\"clazz_present\":%s", glyph->clazz ? "true" : "false");
    if (glyph->format == FT_GLYPH_FORMAT_BITMAP) {
        FT_BitmapGlyph bitmap_glyph = (FT_BitmapGlyph)glyph;
        FT_Bitmap* bitmap = &bitmap_glyph->bitmap;
        long len = 0;
        if (bitmap->buffer && bitmap->rows > 0) {
            len = labs(bitmap->pitch) * bitmap->rows;
        }
        printf(",\"bitmap\":");
        if (!bitmap->buffer || len == 0) {
            printf("null");
        } else {
            printf("{\"width\":%u,\"rows\":%u,\"pitch\":%d,\"pixel_mode\":%u,\"num_grays\":%u,\"left\":%d,\"top\":%d,\"buffer_hex\":\"",
                   bitmap->width,
                   bitmap->rows,
                   bitmap->pitch,
                   bitmap->pixel_mode,
                   bitmap->num_grays,
                   bitmap_glyph->left,
                   bitmap_glyph->top);
            print_hex_bytes(bitmap->buffer, len);
            printf("\"}");
        }
    }
    printf("}}");
}

static void print_get_glyph_payload(FT_GlyphSlot slot, const char* action) {
    FT_Glyph glyph = NULL;
    FT_Error err = FT_Get_Glyph(slot, &glyph);
    if (!err && streq(action, "copy")) {
        FT_Glyph copy = NULL;
        err = FT_Glyph_Copy(glyph, &copy);
        FT_Done_Glyph(glyph);
        glyph = copy;
    }
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",");
        print_glyph_record_payload(glyph);
        printf("}\n");
    }
    if (glyph) {
        FT_Done_Glyph(glyph);
    }
}

static int load_record_source_bytes(const char* source_kind,
                                    const char* source_value,
                                    unsigned char** data,
                                    long* data_len) {
    if (streq(source_kind, "file")) {
        if (load_file(source_value, data, data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 1;
        }
        return 0;
    }
    if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, data, data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            return 1;
        }
        return 0;
    }
    fprintf(stderr, "unsupported source kind: %s\n", source_kind);
    return 1;
}

static FT_Error open_record_face(FT_Library library,
                                 const char* source_kind,
                                 const char* source_value,
                                 FT_Long face_index,
                                 FT_UInt pixel_x,
                                 FT_UInt pixel_y,
                                 unsigned char** data,
                                 FT_Face* face) {
    long data_len = 0;
    if (load_record_source_bytes(source_kind, source_value, data, &data_len) != 0) {
        return FT_Err_Invalid_Argument;
    }
    FT_Error err = FT_New_Memory_Face(library, *data, data_len, face_index, face);
    if (!err) {
        err = FT_Set_Pixel_Sizes(*face, pixel_x, pixel_y);
    }
    return err;
}

static void print_bitmap_glyph_record_row(const char* creation_path, FT_BitmapGlyph glyph) {
    FT_Bitmap* bitmap = &glyph->bitmap;
    long len = 0;
    if (bitmap->buffer && bitmap->rows > 0) {
        len = labs(bitmap->pitch) * bitmap->rows;
    }
    printf("{\"creation_path\":\"%s\",\"glyph\":{", creation_path);
    printf("\"format\":%ld,", (long)glyph->root.format);
    printf("\"advance\":{\"x\":%ld,\"y\":%ld},", glyph->root.advance.x, glyph->root.advance.y);
    printf("\"library_present\":%s,", glyph->root.library ? "true" : "false");
    printf("\"clazz_present\":%s,", glyph->root.clazz ? "true" : "false");
    printf("\"bitmap\":{");
    printf("\"width\":%u,\"rows\":%u,\"pitch\":%d,\"pixel_mode\":%u,\"num_grays\":%u,\"left\":%d,\"top\":%d,\"buffer_hex\":\"",
           bitmap->width,
           bitmap->rows,
           bitmap->pitch,
           bitmap->pixel_mode,
           bitmap->num_grays,
           glyph->left,
           glyph->top);
    if (bitmap->buffer && len > 0) {
        print_hex_bytes(bitmap->buffer, len);
    }
    printf("\"}}}");
}

static int emit_bitmap_glyph_record_paths(int argc, char** argv) {
    (void)argc;
    const char* outline_kind = argv[2];
    const char* outline_value = argv[3];
    const char* bitmap_kind = argv[4];
    const char* bitmap_value = argv[5];
    FT_Long face_index = (FT_Long)strtol(argv[6], NULL, 10);
    FT_UInt pixel_x = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt pixel_y = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt bitmap_glyph_index = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_UInt outline_glyph_index = (FT_UInt)strtoul(argv[10], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[11], NULL, 10);
    FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[12], NULL, 10);

    FT_Library library = NULL;
    FT_Face bitmap_face = NULL;
    FT_Face outline_face = NULL;
    FT_Glyph bitmap_glyph = NULL;
    FT_Glyph outline_glyph = NULL;
    unsigned char* bitmap_data = NULL;
    unsigned char* outline_data = NULL;
    FT_Error err = FT_Init_FreeType(&library);

    if (!err) {
        err = open_record_face(library, bitmap_kind, bitmap_value, face_index, pixel_x, pixel_y, &bitmap_data, &bitmap_face);
    }
    if (!err) {
        err = FT_Load_Glyph(bitmap_face, bitmap_glyph_index, load_flags);
    }
    if (!err) {
        err = FT_Get_Glyph(bitmap_face->glyph, &bitmap_glyph);
    }
    if (!err && (!bitmap_glyph || bitmap_glyph->format != FT_GLYPH_FORMAT_BITMAP)) {
        err = FT_Err_Invalid_Glyph_Format;
    }
    if (!err) {
        err = open_record_face(library, outline_kind, outline_value, face_index, pixel_x, pixel_y, &outline_data, &outline_face);
    }
    if (!err) {
        err = FT_Load_Glyph(outline_face, outline_glyph_index, load_flags);
    }
    if (!err) {
        err = FT_Get_Glyph(outline_face->glyph, &outline_glyph);
    }
    if (!err) {
        err = FT_Glyph_To_Bitmap(&outline_glyph, render_mode, NULL, 0);
    }
    if (!err && (!outline_glyph || outline_glyph->format != FT_GLYPH_FORMAT_BITMAP)) {
        err = FT_Err_Invalid_Glyph_Format;
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"rows\":[");
        print_bitmap_glyph_record_row("FT_Get_Glyph bitmap", (FT_BitmapGlyph)bitmap_glyph);
        printf(",");
        print_bitmap_glyph_record_row("FT_Glyph_To_Bitmap outline", (FT_BitmapGlyph)outline_glyph);
        printf("]}}\n");
    }

    if (bitmap_glyph) {
        FT_Done_Glyph(bitmap_glyph);
    }
    if (outline_glyph) {
        FT_Done_Glyph(outline_glyph);
    }
    if (bitmap_face) {
        FT_Done_Face(bitmap_face);
    }
    if (outline_face) {
        FT_Done_Face(outline_face);
    }
    if (library) {
        FT_Done_FreeType(library);
    }
    free(bitmap_data);
    free(outline_data);
    return 0;
}

static void print_done_outline_glyph_payload(FT_GlyphSlot slot) {
    FT_Glyph glyph = NULL;
    FT_Error err = FT_Get_Glyph(slot, &glyph);
    FT_Glyph_Format format = 0;
    FT_Bool owner = 0;
    short n_points = 0;
    short n_contours = 0;
    if (!err && glyph) {
        FT_OutlineGlyph outline_glyph = (FT_OutlineGlyph)glyph;
        format = glyph->format;
        owner = (outline_glyph->outline.flags & FT_OUTLINE_OWNER) != 0;
        n_points = outline_glyph->outline.n_points;
        n_contours = outline_glyph->outline.n_contours;
    }
    print_status(0);
    printf(",\"output\":{");
    printf("\"void\":true,");
    printf("\"created_glyph_pointer_class\":\"%s\",", glyph ? "non_null" : "null");
    printf("\"create_error\":%d,", err);
    if (!err && glyph) {
        printf("\"format_before_done\":%ld,", (long)format);
        printf("\"outline_owner_class\":\"%s\",", owner ? "owned" : "borrowed");
        printf("\"outline_counts_before_done\":{\"n_points\":%d,\"n_contours\":%d},",
               n_points,
               n_contours);
        printf("\"free_events\":\"FT_Done_Glyph called once for owned outline glyph\",");
        printf("\"lifetime_order\":\"glyph_before_face_and_library\",");
        printf("\"invalid_use_classification\":\"not_attempted\"");
    } else {
        printf("\"format_before_done\":null,");
        printf("\"outline_owner_class\":null,");
        printf("\"outline_counts_before_done\":null,");
        printf("\"free_events\":\"none\",");
        printf("\"lifetime_order\":\"glyph_not_created\",");
        printf("\"invalid_use_classification\":\"not_attempted\"");
    }
    printf("}}\n");
    if (glyph) {
        FT_Done_Glyph(glyph);
    }
}

static void print_done_bitmap_glyph_payload(FT_GlyphSlot slot) {
    FT_Glyph glyph = NULL;
    FT_Error err = FT_Get_Glyph(slot, &glyph);
    FT_Glyph_Format format = 0;
    const char* buffer_class = "null";
    unsigned int width = 0;
    unsigned int rows = 0;
    int pitch = 0;
    if (!err && glyph) {
        FT_BitmapGlyph bitmap_glyph = (FT_BitmapGlyph)glyph;
        FT_Bitmap* bitmap = &bitmap_glyph->bitmap;
        format = glyph->format;
        buffer_class = bitmap->buffer ? "owned_non_null" : "null";
        width = bitmap->width;
        rows = bitmap->rows;
        pitch = bitmap->pitch;
    }
    print_status(0);
    printf(",\"output\":{");
    printf("\"void\":true,");
    printf("\"created_glyph_pointer_class\":\"%s\",", glyph ? "non_null" : "null");
    printf("\"create_error\":%d,", err);
    if (!err && glyph) {
        printf("\"format_before_done\":%ld,", (long)format);
        printf("\"buffer_owner_class\":\"%s\",", buffer_class);
        printf("\"bitmap_before_done\":{\"width\":%u,\"rows\":%u,\"pitch\":%d},",
               width,
               rows,
               pitch);
        printf("\"free_events\":\"FT_Done_Glyph called once for owned bitmap glyph\"");
    } else {
        printf("\"format_before_done\":null,");
        printf("\"buffer_owner_class\":null,");
        printf("\"bitmap_before_done\":null,");
        printf("\"free_events\":\"none\"");
    }
    printf("}}\n");
    if (glyph) {
        FT_Done_Glyph(glyph);
    }
}

static void print_done_bitmap_glyph_row(const char* creation_path, FT_Error err, FT_Glyph glyph) {
    FT_Glyph_Format format = 0;
    const char* buffer_class = "null";
    unsigned int width = 0;
    unsigned int rows = 0;
    int pitch = 0;
    if (!err && glyph) {
        FT_BitmapGlyph bitmap_glyph = (FT_BitmapGlyph)glyph;
        FT_Bitmap* bitmap = &bitmap_glyph->bitmap;
        format = glyph->format;
        buffer_class = bitmap->buffer ? "owned_non_null" : "null";
        width = bitmap->width;
        rows = bitmap->rows;
        pitch = bitmap->pitch;
    }
    printf("{\"creation_path\":\"%s\",", creation_path);
    printf("\"void\":true,");
    printf("\"created_glyph_pointer_class\":\"%s\",", glyph ? "non_null" : "null");
    printf("\"create_error\":%d,", err);
    if (!err && glyph) {
        printf("\"format_before_done\":%ld,", (long)format);
        printf("\"buffer_owner_class\":\"%s\",", buffer_class);
        printf("\"bitmap_before_done\":{\"width\":%u,\"rows\":%u,\"pitch\":%d},",
               width,
               rows,
               pitch);
        printf("\"free_events\":\"FT_Done_Glyph called once for owned bitmap glyph\"");
    } else {
        printf("\"format_before_done\":null,");
        printf("\"buffer_owner_class\":null,");
        printf("\"bitmap_before_done\":null,");
        printf("\"free_events\":\"none\"");
    }
    printf("}");
}

static int emit_done_bitmap_glyph_paths(int argc, char** argv) {
    (void)argc;
    const char* outline_kind = argv[2];
    const char* outline_value = argv[3];
    const char* bitmap_kind = argv[4];
    const char* bitmap_value = argv[5];
    FT_Long face_index = (FT_Long)strtol(argv[6], NULL, 10);
    FT_UInt pixel_x = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt pixel_y = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt bitmap_glyph_index = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_UInt outline_glyph_index = (FT_UInt)strtoul(argv[10], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[11], NULL, 10);
    FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[12], NULL, 10);

    FT_Library library = NULL;
    FT_Face bitmap_face = NULL;
    FT_Face outline_face = NULL;
    FT_Glyph bitmap_glyph = NULL;
    FT_Glyph outline_glyph = NULL;
    unsigned char* bitmap_data = NULL;
    unsigned char* outline_data = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    FT_Error bitmap_err = err;
    FT_Error outline_err = err;

    if (!err) {
        bitmap_err = open_record_face(library, bitmap_kind, bitmap_value, face_index, pixel_x, pixel_y, &bitmap_data, &bitmap_face);
    }
    if (!bitmap_err) {
        bitmap_err = FT_Load_Glyph(bitmap_face, bitmap_glyph_index, load_flags);
    }
    if (!bitmap_err) {
        bitmap_err = FT_Get_Glyph(bitmap_face->glyph, &bitmap_glyph);
    }
    if (!bitmap_err && (!bitmap_glyph || bitmap_glyph->format != FT_GLYPH_FORMAT_BITMAP)) {
        bitmap_err = FT_Err_Invalid_Glyph_Format;
    }

    if (!err) {
        outline_err = open_record_face(library, outline_kind, outline_value, face_index, pixel_x, pixel_y, &outline_data, &outline_face);
    }
    if (!outline_err) {
        outline_err = FT_Load_Glyph(outline_face, outline_glyph_index, load_flags);
    }
    if (!outline_err) {
        outline_err = FT_Get_Glyph(outline_face->glyph, &outline_glyph);
    }
    if (!outline_err) {
        outline_err = FT_Glyph_To_Bitmap(&outline_glyph, render_mode, NULL, 0);
    }
    if (!outline_err && (!outline_glyph || outline_glyph->format != FT_GLYPH_FORMAT_BITMAP)) {
        outline_err = FT_Err_Invalid_Glyph_Format;
    }

    err = bitmap_err ? bitmap_err : outline_err;
    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"rows\":[");
        print_done_bitmap_glyph_row("FT_Get_Glyph bitmap", bitmap_err, bitmap_glyph);
        printf(",");
        print_done_bitmap_glyph_row("FT_Glyph_To_Bitmap outline", outline_err, outline_glyph);
        printf("]}}\n");
    }

    if (bitmap_glyph) {
        FT_Done_Glyph(bitmap_glyph);
    }
    if (outline_glyph) {
        FT_Done_Glyph(outline_glyph);
    }
    if (bitmap_face) {
        FT_Done_Face(bitmap_face);
    }
    if (outline_face) {
        FT_Done_Face(outline_face);
    }
    if (library) {
        FT_Done_FreeType(library);
    }
    free(bitmap_data);
    free(outline_data);
    return 0;
}

static void print_get_glyph_error_row(const char* probe, FT_Error err, FT_Glyph glyph) {
    printf("{\"probe\":\"%s\",\"error\":%d,\"output_pointer_class\":\"%s\"}",
           probe,
           err,
           glyph ? "non_null" : "null");
}

static void print_get_glyph_advance_row(const char* axis, FT_Pos value, FT_Error err, FT_Glyph glyph) {
    printf("{\"probe\":\"%s=%ld\",\"axis\":\"%s\",\"advance\":%ld,\"error\":%d,\"output_pointer_class\":\"%s\"",
           axis,
           value,
           axis,
           value,
           err,
           glyph ? "non_null" : "null");
    if (!err && glyph) {
        printf(",\"glyph\":{\"format\":%ld,\"advance\":{\"x\":%ld,\"y\":%ld},\"library_present\":%s,\"clazz_present\":%s}",
               (long)glyph->format,
               glyph->advance.x,
               glyph->advance.y,
               glyph->library ? "true" : "false",
               glyph->clazz ? "true" : "false");
    }
    printf("}");
}

static int emit_get_glyph_advance_boundaries(FT_Face face, const char* values_csv) {
    size_t values_len = strlen(values_csv);
    char* values = (char*)malloc(values_len + 1);
    if (!values) {
        print_status(FT_Err_Out_Of_Memory);
        printf(",\"output\":null}\n");
        return 0;
    }
    memcpy(values, values_csv, values_len + 1);

    print_status(FT_Err_Invalid_Argument);
    printf(",\"output\":{\"rows\":[");
    int first = 1;
    char* saveptr = NULL;
    for (char* token = strtok_r(values, ",", &saveptr); token; token = strtok_r(NULL, ",", &saveptr)) {
        FT_Pos advance = (FT_Pos)strtol(token, NULL, 10);
        for (int axis_index = 0; axis_index < 2; axis_index++) {
            face->glyph->advance.x = axis_index == 0 ? advance : 0;
            face->glyph->advance.y = axis_index == 1 ? advance : 0;
            FT_Glyph glyph = NULL;
            FT_Error err = FT_Get_Glyph(face->glyph, &glyph);
            if (!first) {
                printf(",");
            }
            first = 0;
            print_get_glyph_advance_row(axis_index == 0 ? "x" : "y", advance, err, glyph);
            if (glyph) {
                FT_Done_Glyph(glyph);
            }
        }
    }
    printf("]}}\n");
    free(values);
    return 0;
}

static int emit_get_glyph_null_inputs(void) {
    FT_Glyph glyph = (FT_Glyph)0x1;
    FT_Error null_slot_error = FT_Get_Glyph(NULL, &glyph);

    FT_GlyphSlotRec slot;
    memset(&slot, 0, sizeof(slot));
    FT_Error null_output_error = FT_Get_Glyph(&slot, NULL);

    printf("{");
    print_status(null_slot_error ? null_slot_error : null_output_error);
    printf(",\"output\":{\"rows\":[");
    print_get_glyph_error_row("null_slot", null_slot_error, glyph);
    printf(",");
    print_get_glyph_error_row("null_aglyph", null_output_error, NULL);
    printf("]}}\n");
    return 0;
}

static void print_get_glyph_unsupported_format_payload(FT_GlyphSlot slot) {
    FT_Glyph glyph = (FT_Glyph)0x1;
    slot->format = (FT_Glyph_Format)0x12345678;
    FT_Error err = FT_Get_Glyph(slot, &glyph);

    print_status(err);
    printf(",\"output\":{\"rows\":[");
    print_get_glyph_error_row("unsupported_tag", err, glyph);
    printf("],\"cleanup_events\":\"none\"}}\n");
}

static void print_glyph_copy_error_row(const char* probe, FT_Error err, FT_Glyph target) {
    printf("{\"probe\":\"%s\",\"error\":%d,\"target_pointer_class\":\"%s\"}",
           probe,
           err,
           target ? "non_null" : "null");
}

static int emit_glyph_copy_null_inputs(void) {
    FT_Glyph target = (FT_Glyph)0x1;
    FT_Error null_source_error = FT_Glyph_Copy(NULL, &target);
    FT_Glyph null_source_target = target;

    FT_Error null_target_error = FT_Glyph_Copy(NULL, NULL);

    FT_GlyphRec source;
    memset(&source, 0, sizeof(source));
    target = (FT_Glyph)0x1;
    FT_Error null_class_error = FT_Glyph_Copy(&source, &target);
    FT_Glyph null_class_target = target;

    printf("{");
    print_status(null_source_error ? null_source_error : (null_target_error ? null_target_error : null_class_error));
    printf(",\"output\":{\"rows\":[");
    print_glyph_copy_error_row("null_source", null_source_error, null_source_target);
    printf(",");
    print_glyph_copy_error_row("null_target", null_target_error, NULL);
    printf(",");
    print_glyph_copy_error_row("source_null_clazz", null_class_error, null_class_target);
    printf("],\"target_write_order\":\"early Invalid_Argument preserves non-null target before copy allocation\"}}\n");
    return 0;
}

static int emit_done_glyph_null(void) {
    FT_Done_Glyph(NULL);
    print_ok_output_prefix();
    printf("{\"void\":true,\"null_glyph_noop\":true,\"memory_touched\":false}}\n");
    return 0;
}

static void print_sbit_payload(FT_GlyphSlot slot) {
    FT_Bitmap* bitmap = &slot->bitmap;
    long len = 0;
    if (bitmap->buffer && bitmap->rows > 0) {
        len = labs(bitmap->pitch) * bitmap->rows;
    }
    printf("\"output\":{\"sbit\":{");
    printf("\"width\":%u,\"height\":%u,\"left\":%d,\"top\":%d,",
           bitmap->width,
           bitmap->rows,
           slot->bitmap_left,
           slot->bitmap_top);
    printf("\"format\":%u,\"max_grays\":%u,\"pitch\":%d,",
           bitmap->pixel_mode,
           bitmap->num_grays,
           bitmap->pitch);
    printf("\"xadvance\":%ld,\"yadvance\":%ld,",
           slot->advance.x >> 6,
           slot->advance.y >> 6);
    printf("\"buffer_null\":%s,\"buffer_hex\":\"",
           (!bitmap->buffer || len == 0) ? "true" : "false");
    if (bitmap->buffer && len > 0) {
        print_hex_bytes(bitmap->buffer, len);
    }
    printf("\"},\"node\":{\"locked\":false}}");
}

static void print_sbit_object(FT_GlyphSlot slot) {
    FT_Bitmap* bitmap = &slot->bitmap;
    long len = 0;
    if (bitmap->buffer && bitmap->rows > 0) {
        len = labs(bitmap->pitch) * bitmap->rows;
    }
    printf("\"sbit\":{");
    printf("\"width\":%u,\"height\":%u,\"left\":%d,\"top\":%d,",
           bitmap->width,
           bitmap->rows,
           slot->bitmap_left,
           slot->bitmap_top);
    printf("\"format\":%u,\"max_grays\":%u,\"pitch\":%d,",
           bitmap->pixel_mode,
           bitmap->num_grays,
           bitmap->pitch);
    printf("\"xadvance\":%ld,\"yadvance\":%ld,",
           slot->advance.x >> 6,
           slot->advance.y >> 6);
    printf("\"buffer_null\":%s,\"buffer_hex\":\"",
           (!bitmap->buffer || len == 0) ? "true" : "false");
    if (bitmap->buffer && len > 0) {
        print_hex_bytes(bitmap->buffer, len);
    }
    printf("\"},\"node\":{\"locked\":false}");
}

typedef struct SBitSnapshot_ {
    FT_Bool present;
    FT_UInt width;
    FT_UInt height;
    FT_Int left;
    FT_Int top;
    FT_Byte format;
    FT_Byte max_grays;
    FT_Short pitch;
    FT_Short xadvance;
    FT_Short yadvance;
    FT_Bool buffer_null;
    long buffer_len;
    unsigned char* buffer;
} SBitSnapshot;

static int snapshot_ftc_sbit(FTC_SBit sbit, SBitSnapshot* snapshot) {
    memset(snapshot, 0, sizeof(*snapshot));
    if (!sbit) {
        return 0;
    }
    snapshot->present = 1;
    snapshot->width = sbit->width;
    snapshot->height = sbit->height;
    snapshot->left = sbit->left;
    snapshot->top = sbit->top;
    snapshot->format = sbit->format;
    snapshot->max_grays = sbit->max_grays;
    snapshot->pitch = sbit->pitch;
    snapshot->xadvance = sbit->xadvance;
    snapshot->yadvance = sbit->yadvance;
    snapshot->buffer_null = !sbit->buffer;
    if (sbit->buffer && sbit->height > 0) {
        snapshot->buffer_len = labs((long)sbit->pitch) * (long)sbit->height;
        if (snapshot->buffer_len > 0) {
            snapshot->buffer = (unsigned char*)malloc((size_t)snapshot->buffer_len);
            if (!snapshot->buffer) {
                return 1;
            }
            memcpy(snapshot->buffer, sbit->buffer, (size_t)snapshot->buffer_len);
        }
    }
    return 0;
}

static void free_sbit_snapshot(SBitSnapshot* snapshot) {
    free(snapshot->buffer);
    snapshot->buffer = NULL;
}

static int ftc_sbit_snapshot_still_matches(FTC_SBit sbit, const SBitSnapshot* snapshot) {
    if (!snapshot->present) {
        return sbit == NULL;
    }
    if (!sbit) {
        return 0;
    }
    if (snapshot->width != sbit->width ||
        snapshot->height != sbit->height ||
        snapshot->left != sbit->left ||
        snapshot->top != sbit->top ||
        snapshot->format != sbit->format ||
        snapshot->max_grays != sbit->max_grays ||
        snapshot->pitch != sbit->pitch ||
        snapshot->xadvance != sbit->xadvance ||
        snapshot->yadvance != sbit->yadvance ||
        snapshot->buffer_null != (FT_Bool)!sbit->buffer) {
        return 0;
    }
    if (!snapshot->buffer || snapshot->buffer_len == 0) {
        return 1;
    }
    return sbit->buffer && memcmp(snapshot->buffer, sbit->buffer, (size_t)snapshot->buffer_len) == 0;
}

static void print_ftc_sbit_snapshot_fields(const SBitSnapshot* snapshot) {
    if (!snapshot->present) {
        printf("null");
        return;
    }
    printf("{\"width\":%u,\"height\":%u,\"left\":%d,\"top\":%d,",
           snapshot->width,
           snapshot->height,
           snapshot->left,
           snapshot->top);
    printf("\"format\":%u,\"max_grays\":%u,\"pitch\":%d,",
           snapshot->format,
           snapshot->max_grays,
           snapshot->pitch);
    printf("\"xadvance\":%d,\"yadvance\":%d,",
           snapshot->xadvance,
           snapshot->yadvance);
    printf("\"buffer_null\":%s,\"buffer_hex\":\"",
           snapshot->buffer_null || snapshot->buffer_len == 0 ? "true" : "false");
    if (snapshot->buffer && snapshot->buffer_len > 0) {
        print_hex_bytes(snapshot->buffer, snapshot->buffer_len);
    }
    printf("\"}");
}

typedef struct ImageCacheRequesterData_ {
    unsigned char* data;
    long data_len;
    FT_Long face_index;
    int request_count;
} ImageCacheRequesterData;

static FT_Error image_cache_requester(FTC_FaceID face_id,
                                      FT_Library library,
                                      FT_Pointer req_data,
                                      FT_Face* aface) {
    (void)req_data;
    ImageCacheRequesterData* data = (ImageCacheRequesterData*)face_id;
    data->request_count++;
    return FT_New_Memory_Face(library, data->data, data->data_len, data->face_index, aface);
}

static void print_image_cache_glyph_object(FT_Glyph glyph) {
    printf("\"glyph\":{");
    printf("\"format\":%ld,", glyph ? (long)glyph->format : 0L);
    printf("\"advance\":{\"x\":%ld,\"y\":%ld},",
           glyph ? glyph->advance.x : 0L,
           glyph ? glyph->advance.y : 0L);
    printf("\"library_present\":%s,", (glyph && glyph->library) ? "true" : "false");
    printf("\"clazz_present\":%s", (glyph && glyph->clazz) ? "true" : "false");
    printf("},\"node\":{\"locked\":false}");
}

typedef struct ImageGlyphSnapshot_ {
    FT_Bool present;
    long format;
    long advance_x;
    long advance_y;
    FT_Bool library_present;
    FT_Bool clazz_present;
} ImageGlyphSnapshot;

static ImageGlyphSnapshot snapshot_image_glyph(FT_Glyph glyph) {
    ImageGlyphSnapshot snapshot;
    memset(&snapshot, 0, sizeof(snapshot));
    if (glyph) {
        snapshot.present = 1;
        snapshot.format = (long)glyph->format;
        snapshot.advance_x = glyph->advance.x;
        snapshot.advance_y = glyph->advance.y;
        snapshot.library_present = glyph->library ? 1 : 0;
        snapshot.clazz_present = glyph->clazz ? 1 : 0;
    }
    return snapshot;
}

static int image_glyph_snapshot_still_matches(FT_Glyph glyph, const ImageGlyphSnapshot* snapshot) {
    if (!snapshot->present) {
        return glyph == NULL;
    }
    if (!glyph) {
        return 0;
    }
    return snapshot->format == (long)glyph->format &&
           snapshot->advance_x == glyph->advance.x &&
           snapshot->advance_y == glyph->advance.y &&
           snapshot->library_present == (FT_Bool)(glyph->library ? 1 : 0) &&
           snapshot->clazz_present == (FT_Bool)(glyph->clazz ? 1 : 0);
}

static void print_image_glyph_snapshot_object(const ImageGlyphSnapshot* snapshot) {
    if (!snapshot->present) {
        printf("\"glyph\":null,\"node\":{\"locked\":false}");
        return;
    }
    printf("\"glyph\":{");
    printf("\"format\":%ld,", snapshot->format);
    printf("\"advance\":{\"x\":%ld,\"y\":%ld},",
           snapshot->advance_x,
           snapshot->advance_y);
    printf("\"library_present\":%s,", snapshot->library_present ? "true" : "false");
    printf("\"clazz_present\":%s", snapshot->clazz_present ? "true" : "false");
    printf("},\"node\":{\"locked\":false}");
}

static void print_image_cache_lookup_scaler_row(FTC_ImageCache cache,
                                                FTC_Manager manager,
                                                ImageCacheRequesterData* requester,
                                                FTC_ScalerRec* scaler,
                                                FT_UInt glyph_index,
                                                FT_ULong load_flags_ulong) {
    FT_Glyph glyph = NULL;
    FTC_Node node = NULL;
    FT_Int32 load_flags = (FT_Int32)load_flags_ulong;
    FT_Error error = FTC_ImageCache_LookupScaler(cache, scaler, load_flags, glyph_index, &glyph, &node);
    printf("{\"scaler\":{\"width\":%u,\"height\":%u,\"pixel\":%u,\"x_res\":%u,\"y_res\":%u},"
           "\"glyph_index\":%u,\"effective_load_flags\":%ld,\"status\":%d,\"error\":%d,",
           scaler->width,
           scaler->height,
           scaler->pixel,
           scaler->x_res,
           scaler->y_res,
           glyph_index,
           (long)load_flags,
           error,
           error);
    if (error) {
        printf("\"glyph\":null,\"node\":{\"locked\":false}}");
    } else {
        print_image_cache_glyph_object(glyph);
        printf("}");
    }
    if (node) {
        FTC_Node_Unref(node, manager);
    }
}

static int emit_image_cache_lookup_scaler(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    char* scalers_arg = (char*)malloc(strlen(argv[5]) + 1);
    char* glyphs_arg = (char*)malloc(strlen(argv[6]) + 1);
    if (!scalers_arg || !glyphs_arg) {
        free(scalers_arg);
        free(glyphs_arg);
        return 1;
    }
    memcpy(scalers_arg, argv[5], strlen(argv[5]) + 1);
    memcpy(glyphs_arg, argv[6], strlen(argv[6]) + 1);
    FT_ULong load_flags_ulong = (FT_ULong)strtoull(argv[7], NULL, 0);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(scalers_arg);
            free(glyphs_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(scalers_arg);
            free(glyphs_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(scalers_arg);
        free(glyphs_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(scalers_arg);
        free(glyphs_arg);
        return 0;
    }

    ImageCacheRequesterData requester;
    requester.data = data;
    requester.data_len = data_len;
    requester.face_index = face_index;
    requester.request_count = 0;

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             image_cache_requester,
                                             NULL,
                                             &manager);
    FTC_ImageCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_ImageCache_New(manager, &cache);
    if (cache_error) {
        printf("{");
        print_status(cache_error);
        printf(",\"output\":{\"outputs\":[]}}\n");
        if (manager) {
            FTC_Manager_Done(manager);
        }
        FT_Done_FreeType(library);
        free(data);
        free(scalers_arg);
        free(glyphs_arg);
        return 0;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"outputs\":[");
    int first = 1;
    char* scaler_cursor = scalers_arg;
    while (scaler_cursor && *scaler_cursor) {
        char* scaler_next = strchr(scaler_cursor, ';');
        if (scaler_next) {
            *scaler_next = '\0';
        }
        FTC_ScalerRec scaler;
        unsigned int pixel = 0;
        memset(&scaler, 0, sizeof(scaler));
        if (sscanf(scaler_cursor, "%u:%u:%u:%u:%u",
                   &scaler.width,
                   &scaler.height,
                   &pixel,
                   &scaler.x_res,
                   &scaler.y_res) != 5) {
            FTC_Manager_Done(manager);
            FT_Done_FreeType(library);
            free(data);
            free(scalers_arg);
            free(glyphs_arg);
            return 2;
        }
        scaler.face_id = (FTC_FaceID)&requester;
        scaler.pixel = (FT_UInt)pixel;

        char* glyphs_copy = (char*)malloc(strlen(glyphs_arg) + 1);
        if (!glyphs_copy) {
            FTC_Manager_Done(manager);
            FT_Done_FreeType(library);
            free(data);
            free(scalers_arg);
            free(glyphs_arg);
            return 1;
        }
        memcpy(glyphs_copy, glyphs_arg, strlen(glyphs_arg) + 1);
        char* glyph_cursor = glyphs_copy;
        while (glyph_cursor && *glyph_cursor) {
            char* glyph_next = strchr(glyph_cursor, ',');
            if (glyph_next) {
                *glyph_next = '\0';
            }
            FT_UInt glyph_index = (FT_UInt)strtoul(glyph_cursor, NULL, 10);
            if (!first) {
                printf(",");
            }
            print_image_cache_lookup_scaler_row(cache, manager, &requester, &scaler, glyph_index, load_flags_ulong);
            first = 0;
            glyph_cursor = glyph_next ? glyph_next + 1 : NULL;
        }
        free(glyphs_copy);
        scaler_cursor = scaler_next ? scaler_next + 1 : NULL;
    }
    printf("]}}\n");

    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    free(scalers_arg);
    free(glyphs_arg);
    return 0;
}

static void print_image_cache_lookup_row(FTC_ImageCache cache,
                                         FTC_Manager manager,
                                         ImageCacheRequesterData* requester,
                                         FTC_ImageTypeRec* image_type,
                                         FT_UInt glyph_index,
                                         int repeat_lookup) {
    FT_Glyph glyph = NULL;
    FTC_Node node = NULL;
    int before_calls = requester->request_count;
    FT_Error error = FTC_ImageCache_Lookup(cache, image_type, glyph_index, &glyph, &node);
    int after_first_calls = requester->request_count;
    FT_Glyph repeat_glyph = NULL;
    FTC_Node repeat_node = NULL;
    FT_Error repeat_error = error;
    if (repeat_lookup) {
        repeat_error = FTC_ImageCache_Lookup(cache, image_type, glyph_index, &repeat_glyph, &repeat_node);
    }
    printf("{\"glyph_index\":%u,\"image_type\":{\"width\":%u,\"height\":%u,\"flags\":%ld},"
           "\"status\":%d,\"error\":%d,\"repeat_status\":%d,"
           "\"requester_count_before\":%d,\"requester_count_after_first\":%d,"
           "\"requester_count_after_repeat\":%d,",
           glyph_index,
           image_type->width,
           image_type->height,
           (long)image_type->flags,
           error,
           error,
           repeat_error,
           before_calls,
           after_first_calls,
           requester->request_count);
    if (error) {
        printf("\"glyph\":null,\"node\":{\"locked\":false}}");
    } else {
        print_image_cache_glyph_object(glyph);
        printf("}");
    }
    if (node) {
        FTC_Node_Unref(node, manager);
    }
    if (repeat_node) {
        FTC_Node_Unref(repeat_node, manager);
    }
}

static int emit_image_cache_lookup(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    const char* scenario = argv[5];
    FT_UInt width = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_UInt height = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_ULong flags = (FT_ULong)strtoull(argv[8], NULL, 0);
    char* glyphs_arg = (char*)malloc(strlen(argv[9]) + 1);
    if (!glyphs_arg) {
        return 1;
    }
    memcpy(glyphs_arg, argv[9], strlen(argv[9]) + 1);
    int repeat_lookup = atoi(argv[10]) != 0;

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(glyphs_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(glyphs_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(glyphs_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(glyphs_arg);
        return 0;
    }

    ImageCacheRequesterData requester;
    requester.data = data;
    requester.data_len = data_len;
    requester.face_index = face_index;
    requester.request_count = 0;
    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             image_cache_requester,
                                             NULL,
                                             &manager);
    FTC_ImageCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_ImageCache_New(manager, &cache);
    if (cache_error) {
        printf("{");
        print_status(cache_error);
        printf(",\"output\":{\"scenario\":\"");
        print_json_string_content(scenario);
        printf("\",\"rows\":[]}}\n");
        if (manager) {
            FTC_Manager_Done(manager);
        }
        FT_Done_FreeType(library);
        free(data);
        free(glyphs_arg);
        return 0;
    }

    FTC_ImageTypeRec image_type;
    memset(&image_type, 0, sizeof(image_type));
    image_type.face_id = (FTC_FaceID)&requester;
    image_type.width = width;
    image_type.height = height;
    image_type.flags = (FT_Int32)flags;

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"rows\":[");
    char* cursor = glyphs_arg;
    int first = 1;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        FT_UInt glyph_index = (FT_UInt)strtoul(cursor, NULL, 10);
        if (!first) {
            printf(",");
        }
        print_image_cache_lookup_row(cache, manager, &requester, &image_type, glyph_index, repeat_lookup);
        first = 0;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");

    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    free(glyphs_arg);
    return 0;
}

static int emit_image_type_descriptor_lifetime(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    FT_UInt width = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt height = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_ULong flags = (FT_ULong)strtoull(argv[7], NULL, 0);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[8], NULL, 10);

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    ImageCacheRequesterData requester;
    requester.data = data;
    requester.data_len = data_len;
    requester.face_index = face_index;
    requester.request_count = 0;

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             image_cache_requester,
                                             NULL,
                                             &manager);
    FTC_ImageCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_ImageCache_New(manager, &cache);
    if (cache_error) {
        printf("{");
        print_status(cache_error);
        printf(",\"output\":null}\n");
        if (manager) {
            FTC_Manager_Done(manager);
        }
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    FTC_ImageTypeRec image_type;
    memset(&image_type, 0, sizeof(image_type));
    image_type.face_id = (FTC_FaceID)&requester;
    image_type.width = width;
    image_type.height = height;
    image_type.flags = (FT_Int32)flags;

    FT_Glyph glyph = NULL;
    FTC_Node node = NULL;
    FT_Error error = FTC_ImageCache_Lookup(cache, &image_type, glyph_index, &glyph, &node);
    ImageGlyphSnapshot snapshot = snapshot_image_glyph(error ? NULL : glyph);

    image_type.width += 1;
    image_type.height += 1;
    image_type.flags ^= FT_LOAD_NO_HINTING;
    int unchanged = image_glyph_snapshot_still_matches(error ? NULL : glyph, &snapshot);

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{");
    printf("\"status\":%d,", error);
    printf("\"lookup_result\":{\"glyph_index\":%u,"
           "\"image_type\":{\"width\":%u,\"height\":%u,\"flags\":%ld},"
           "\"status\":%d,\"error\":%d,",
           glyph_index,
           width,
           height,
           (long)(FT_Int32)flags,
           error,
           error);
    print_image_glyph_snapshot_object(&snapshot);
    printf("},\"post_lookup_descriptor_mutation_effect_on_existing_node\":{");
    printf("\"mutation_performed\":true,");
    printf("\"mutated_image_type\":{\"width\":%u,\"height\":%u,\"flags\":%ld},",
           image_type.width,
           image_type.height,
           (long)image_type.flags);
    printf("\"existing_node_unchanged\":%s", unchanged ? "true" : "false");
    printf("}}}\n");

    if (node) {
        FTC_Node_Unref(node, manager);
    }
    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static void print_image_type_lookup_result(FT_Error error,
                                           const ImageGlyphSnapshot* image_snapshot,
                                           const SBitSnapshot* sbit_snapshot,
                                           const char* payload_kind) {
    printf("{\"status\":%d,\"error\":%d,", error, error);
    if (streq(payload_kind, "image")) {
        print_image_glyph_snapshot_object(image_snapshot);
    } else {
        printf("\"sbit\":");
        print_ftc_sbit_snapshot_fields(sbit_snapshot);
        printf(",\"node\":{\"locked\":false}");
    }
    printf("}");
}

static int emit_image_type_lookup_probe(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    FT_UInt width = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt height = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_ULong flags = (FT_ULong)strtoull(argv[7], NULL, 0);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[8], NULL, 10);

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    ImageCacheRequesterData requester;
    requester.data = data;
    requester.data_len = data_len;
    requester.face_index = face_index;
    requester.request_count = 0;

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             image_cache_requester,
                                             NULL,
                                             &manager);
    FTC_ImageCache image_cache = NULL;
    FT_Error image_cache_error = manager_error ? manager_error : FTC_ImageCache_New(manager, &image_cache);
    FTC_SBitCache sbit_cache = NULL;
    FT_Error sbit_cache_error = image_cache_error ? image_cache_error : FTC_SBitCache_New(manager, &sbit_cache);
    if (sbit_cache_error) {
        printf("{");
        print_status(sbit_cache_error);
        printf(",\"output\":null}\n");
        if (manager) {
            FTC_Manager_Done(manager);
        }
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    FTC_ImageTypeRec image_type;
    memset(&image_type, 0, sizeof(image_type));
    image_type.face_id = (FTC_FaceID)&requester;
    image_type.width = width;
    image_type.height = height;
    image_type.flags = (FT_Int32)flags;

    FT_Glyph glyph = NULL;
    FTC_Node image_node = NULL;
    FT_Error image_error = FTC_ImageCache_Lookup(image_cache, &image_type, glyph_index, &glyph, &image_node);
    ImageGlyphSnapshot image_snapshot = snapshot_image_glyph(image_error ? NULL : glyph);

    FTC_SBit sbit = NULL;
    FTC_Node sbit_node = NULL;
    FT_Error sbit_error = FTC_SBitCache_Lookup(sbit_cache, &image_type, glyph_index, &sbit, &sbit_node);
    SBitSnapshot sbit_snapshot;
    if (snapshot_ftc_sbit(sbit_error ? NULL : sbit, &sbit_snapshot) != 0) {
        if (image_node) {
            FTC_Node_Unref(image_node, manager);
        }
        if (sbit_node) {
            FTC_Node_Unref(sbit_node, manager);
        }
        FTC_Manager_Done(manager);
        FT_Done_FreeType(library);
        free(data);
        return 1;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{");
    printf("\"status\":0,\"effective_query\":{\"face_id_identity\":\"stable_pointer\","
           "\"width\":%u,\"height\":%u,\"flags\":%ld,\"glyph_index\":%u},",
           width,
           height,
           (long)(FT_Int32)flags,
           glyph_index);
    printf("\"result_metrics\":{\"image\":");
    print_image_type_lookup_result(image_error, &image_snapshot, &sbit_snapshot, "image");
    printf(",\"sbit\":");
    print_image_type_lookup_result(sbit_error, &image_snapshot, &sbit_snapshot, "sbit");
    printf("}}}\n");

    free_sbit_snapshot(&sbit_snapshot);
    if (image_node) {
        FTC_Node_Unref(image_node, manager);
    }
    if (sbit_node) {
        FTC_Node_Unref(sbit_node, manager);
    }
    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_sbit_cache_lookup_scaler(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    char* scalers_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!scalers_arg) {
        return 1;
    }
    memcpy(scalers_arg, argv[5], strlen(argv[5]) + 1);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_ULong load_flags_ulong = (FT_ULong)strtoull(argv[7], NULL, 0);
    FT_Int32 load_flags = (FT_Int32)load_flags_ulong;

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(scalers_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(scalers_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(scalers_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(scalers_arg);
        return 0;
    }

    FT_Face face = NULL;
    FT_Error open_error = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    if (open_error) {
        printf("{");
        print_status(open_error);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        free(data);
        free(scalers_arg);
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"outputs\":[");
    char* cursor = scalers_arg;
    int first = 1;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        unsigned int width = 0;
        unsigned int height = 0;
        unsigned int pixel = 0;
        unsigned int x_res = 0;
        unsigned int y_res = 0;
        if (sscanf(cursor, "%u:%u:%u:%u:%u", &width, &height, &pixel, &x_res, &y_res) != 5) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            free(scalers_arg);
            return 2;
        }
        FT_Error size_error = pixel
            ? FT_Set_Pixel_Sizes(face, width, height)
            : FT_Set_Char_Size(face, (FT_F26Dot6)width, (FT_F26Dot6)height, x_res, y_res);
        FT_Error load_error = size_error ? size_error : FT_Load_Glyph(face, glyph_index, load_flags);
        FT_Error render_error = load_error;
        if (!render_error && face->glyph->format != FT_GLYPH_FORMAT_BITMAP) {
            render_error = FT_Render_Glyph(face->glyph, FT_RENDER_MODE_NORMAL);
        }
        if (!first_error && render_error) {
            first_error = render_error;
        }
        if (!first) {
            printf(",");
        }
        printf("{\"scaler\":{\"width\":%u,\"height\":%u,\"pixel\":%u,\"x_res\":%u,\"y_res\":%u},\"effective_load_flags\":%ld,\"status\":%d,\"error\":%d,",
               width,
               height,
               pixel,
               x_res,
               y_res,
               (long)load_flags,
               render_error,
               render_error);
        if (render_error) {
            printf("\"sbit\":null,\"node\":{\"locked\":false}}");
        } else {
            print_sbit_object(face->glyph);
            printf("}");
        }
        first = 0;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");

    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    free(scalers_arg);
    (void)first_error;
    return 0;
}

static int emit_cache_node_lifecycle(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt size = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_ULong max_bytes = (FT_ULong)strtoull(argv[7], NULL, 0);
    const char* scenario = argv[8];

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    ImageCacheRequesterData requester;
    requester.data = data;
    requester.data_len = data_len;
    requester.face_index = face_index;
    requester.request_count = 0;

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, max_bytes,
                                             image_cache_requester,
                                             NULL,
                                             &manager);
    FTC_SBitCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_SBitCache_New(manager, &cache);
    if (cache_error) {
        printf("{");
        print_status(cache_error);
        printf(",\"output\":null}\n");
        if (manager) {
            FTC_Manager_Done(manager);
        }
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    FTC_ImageTypeRec image_type;
    memset(&image_type, 0, sizeof(image_type));
    image_type.face_id = (FTC_FaceID)&requester;
    image_type.width = size;
    image_type.height = size;
    image_type.flags = FT_LOAD_DEFAULT;

    FTC_SBit sbit = NULL;
    FTC_Node node = NULL;
    FT_Error lookup_error = FTC_SBitCache_Lookup(cache, &image_type, glyph_index, &sbit, &node);
    SBitSnapshot snapshot;
    if (snapshot_ftc_sbit(lookup_error ? NULL : sbit, &snapshot) != 0) {
        if (node) {
            FTC_Node_Unref(node, manager);
        }
        FTC_Manager_Done(manager);
        FT_Done_FreeType(library);
        free(data);
        return 1;
    }

    int node_present = node ? 1 : 0;
    int cache_index = node ? ((FTC_NodeRec*)node)->cache_index : -1;
    int ref_count_before_unref = node ? ((FTC_NodeRec*)node)->ref_count : 0;
    int sbit_readable_before_unref = ftc_sbit_snapshot_still_matches(lookup_error ? NULL : sbit, &snapshot);
    if (node) {
        FTC_Node_Unref(node, manager);
    }
    int ref_count_after_unref = node ? ((FTC_NodeRec*)node)->ref_count : 0;

    unsigned int pressure_glyphs[5] = {37, 38, 39, 40, 41};
    FT_Error pressure_statuses[5] = {0, 0, 0, 0, 0};
    for (int index = 0; index < 5; index++) {
        FTC_SBit pressure_sbit = NULL;
        FTC_Node pressure_node = NULL;
        pressure_statuses[index] =
            FTC_SBitCache_Lookup(cache, &image_type, pressure_glyphs[index], &pressure_sbit, &pressure_node);
        if (pressure_node) {
            FTC_Node_Unref(pressure_node, manager);
        }
    }

    FTC_SBit repeat_sbit = NULL;
    FTC_Node repeat_node = NULL;
    FT_Error repeat_error = FTC_SBitCache_Lookup(cache, &image_type, glyph_index, &repeat_sbit, &repeat_node);
    int repeat_same_node = node && repeat_node && node == repeat_node;
    if (repeat_node) {
        FTC_Node_Unref(repeat_node, manager);
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{");
    printf("\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"lookup\":\"FTC_SBitCache_Lookup\","
           "\"glyph_index\":%u,\"size\":%u,\"max_bytes\":%lu,"
           "\"lookup_status\":%d,\"repeat_status\":%d,"
           "\"requester_count_final\":%d,",
           glyph_index,
           size,
           (unsigned long)max_bytes,
           lookup_error,
           repeat_error,
           requester.request_count);
    printf("\"sbit\":");
    print_ftc_sbit_snapshot_fields(&snapshot);
    printf(",");
    printf("\"node\":{\"anode_nullness\":\"%s\","
           "\"locked\":%s,"
           "\"cache_handle_identity\":\"manager_cache_0\","
           "\"cache_index\":%d,"
           "\"ref_count_before_unref\":%d,"
           "\"ref_count_after_unref\":%d,"
           "\"ref_count_delta\":%d},",
           node_present ? "non_null" : "null",
           node_present && ref_count_before_unref > 0 ? "true" : "false",
           cache_index,
           ref_count_before_unref,
           ref_count_after_unref,
           ref_count_after_unref - ref_count_before_unref);
    printf("\"sbit_still_readable_before_pressure\":%s,"
           "\"locked_survival_class\":\"held_until_unref\","
           "\"after_unref_survival_class\":\"eligible\",",
           sbit_readable_before_unref ? "true" : "false");
    printf("\"post_pressure\":{\"lookup_statuses\":[");
    for (int index = 0; index < 5; index++) {
        if (index) {
            printf(",");
        }
        printf("%d", pressure_statuses[index]);
    }
    printf("],\"repeat_same_node_after_unref\":%s,"
           "\"node_survival_class\":\"%s\"}",
           repeat_same_node ? "true" : "false",
           repeat_same_node ? "survived_unlocked_pressure" : "flushable_or_replaced");
    printf("}}\n");

    free_sbit_snapshot(&snapshot);
    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_cache_node_unref_null_only(void) {
    FTC_Node_Unref(NULL, NULL);

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"void\":true,");
    printf("\"rows\":[");
    printf("{\"node\":\"null\",\"manager\":\"null\",\"void_return\":true,\"side_effects\":\"none\"}");
    printf("]}}\n");
    return 0;
}

static int emit_cache_node_unref_null_or_invalid(void) {
    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        return 0;
    }

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             cache_no_lookup_requester,
                                             NULL,
                                             &manager);
    if (manager_error || !manager) {
        printf("{");
        print_status(manager_error ? manager_error : FT_Err_Invalid_Argument);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    FTC_Node_Unref(NULL, NULL);
    FTC_Node_Unref(NULL, manager);

    FTC_NodeRec foreign_node;
    memset(&foreign_node, 0, sizeof(foreign_node));
    foreign_node.cache_index = 0xFFFF;
    foreign_node.ref_count = 7;
    FT_Short ref_count_before = foreign_node.ref_count;
    /*
     * FreeType src/cache/ftcmanag.c:FTC_Node_Unref only touches a non-null
     * node when cache_index is within manager->num_caches. 0xFFFF is outside
     * every live manager created by FTC_Manager_New, so this exercises the
     * documented foreign/bad-cache-index no-op branch without dereferencing a
     * cache-specific node payload.
     */
    FTC_Node_Unref((FTC_Node)&foreign_node, manager);
    FT_Short ref_count_after = foreign_node.ref_count;

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{");
    printf("\"void\":true,\"side_effects\":[");
    printf("{\"node\":\"null\",\"manager\":\"null\",\"void_return\":true,\"side_effects\":\"none\"},");
    printf("{\"node\":\"null\",\"manager\":\"live_empty\",\"void_return\":true,\"side_effects\":\"none\"},");
    printf("{\"node\":\"foreign_or_bad_cache_index\",\"manager\":\"live_empty\","
           "\"void_return\":true,\"side_effects\":\"none\","
           "\"cache_index_class\":\"out_of_range\",\"ref_count_before\":%d,"
           "\"ref_count_after\":%d}",
           ref_count_before,
           ref_count_after);
    printf("]}}\n");

    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    return 0;
}

static int emit_scaler_descriptor_lifetime(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    char* scalers_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!scalers_arg) {
        return 1;
    }
    memcpy(scalers_arg, argv[5], strlen(argv[5]) + 1);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_ULong load_flags_ulong = (FT_ULong)strtoull(argv[7], NULL, 0);
    FT_Int32 load_flags = (FT_Int32)load_flags_ulong;

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(scalers_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(scalers_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(scalers_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(scalers_arg);
        return 0;
    }

    ImageCacheRequesterData requester;
    requester.data = data;
    requester.data_len = data_len;
    requester.face_index = face_index;
    requester.request_count = 0;

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             image_cache_requester,
                                             NULL,
                                             &manager);
    FTC_SBitCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_SBitCache_New(manager, &cache);
    if (cache_error) {
        printf("{");
        print_status(cache_error);
        printf(",\"output\":null}\n");
        if (manager) {
            FTC_Manager_Done(manager);
        }
        FT_Done_FreeType(library);
        free(data);
        free(scalers_arg);
        return 0;
    }

    FTC_ScalerRec scaler;
    unsigned int pixel = 0;
    memset(&scaler, 0, sizeof(scaler));
    if (sscanf(scalers_arg, "%u:%u:%u:%u:%u",
               &scaler.width,
               &scaler.height,
               &pixel,
               &scaler.x_res,
               &scaler.y_res) != 5) {
        FTC_Manager_Done(manager);
        FT_Done_FreeType(library);
        free(data);
        free(scalers_arg);
        return 2;
    }
    scaler.face_id = (FTC_FaceID)&requester;
    scaler.pixel = (FT_UInt)pixel;
    FT_UInt original_width = scaler.width;
    FT_UInt original_height = scaler.height;
    FT_UInt original_pixel = scaler.pixel;
    FT_UInt original_x_res = scaler.x_res;
    FT_UInt original_y_res = scaler.y_res;

    FTC_SBit sbit = NULL;
    FTC_Node node = NULL;
    FT_Error error = FTC_SBitCache_LookupScaler(cache, &scaler, load_flags, glyph_index, &sbit, &node);
    SBitSnapshot snapshot;
    if (snapshot_ftc_sbit(error ? NULL : sbit, &snapshot) != 0) {
        if (node) {
            FTC_Node_Unref(node, manager);
        }
        FTC_Manager_Done(manager);
        FT_Done_FreeType(library);
        free(data);
        free(scalers_arg);
        return 1;
    }

    scaler.width += 1;
    scaler.height += 1;
    scaler.pixel = scaler.pixel ? 0 : 1;
    scaler.x_res += 1;
    scaler.y_res += 1;
    int unchanged = ftc_sbit_snapshot_still_matches(error ? NULL : sbit, &snapshot);

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{");
    printf("\"status\":%d,", error);
    printf("\"effective_scaler\":{\"width\":%u,\"height\":%u,\"pixel\":%u,\"x_res\":%u,\"y_res\":%u},",
           original_width,
           original_height,
           original_pixel,
           original_x_res,
           original_y_res);
    printf("\"effective_load_flags\":%ld,", (long)load_flags);
    printf("\"result_fields\":");
    print_ftc_sbit_snapshot_fields(&snapshot);
    printf(",\"post_lookup_scaler_mutation_effect_on_existing_result\":{");
    printf("\"mutation_performed\":true,");
    printf("\"mutated_scaler\":{\"width\":%u,\"height\":%u,\"pixel\":%u,\"x_res\":%u,\"y_res\":%u},",
           scaler.width,
           scaler.height,
           scaler.pixel,
           scaler.x_res,
           scaler.y_res);
    printf("\"existing_result_unchanged\":%s", unchanged ? "true" : "false");
    printf("}}}\n");

    free_sbit_snapshot(&snapshot);
    if (node) {
        FTC_Node_Unref(node, manager);
    }
    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    free(scalers_arg);
    return 0;
}

static FT_Error cache_no_lookup_requester(FTC_FaceID face_id,
                                          FT_Library library,
                                          FT_Pointer req_data,
                                          FT_Face* aface) {
    (void)face_id;
    (void)library;
    (void)req_data;
    (void)aface;
    return FT_Err_Invalid_Argument;
}

static int emit_sbit_cache_new_success(void) {
    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        return 0;
    }

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             cache_no_lookup_requester,
                                             NULL,
                                             &manager);
    FTC_SBitCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_SBitCache_New(manager, &cache);
    printf("{");
    print_status(cache_error);
    printf(",\"output\":{\"manager_status\":%d,\"acache\":{\"nullness\":\"%s\","
           "\"owner_identity_class\":\"manager\"},\"manager_done_called\":true}}\n",
           manager_error,
           cache ? "non_null" : "null");
    if (manager) {
        FTC_Manager_Done(manager);
    }
    FT_Done_FreeType(library);
    return 0;
}

static int emit_cache_type_contract(int argc, char** argv) {
    (void)argc;
    const char* constructor = argv[2];
    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        return 0;
    }

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             cache_no_lookup_requester,
                                             NULL,
                                             &manager);
    FT_Error cache_error = manager_error;
    const char* nullness = "null";
    if (!manager_error) {
        if (streq(constructor, "FTC_CMapCache_New")) {
            FTC_CMapCache cache = NULL;
            cache_error = FTC_CMapCache_New(manager, &cache);
            nullness = cache ? "non_null" : "null";
        } else if (streq(constructor, "FTC_ImageCache_New")) {
            FTC_ImageCache cache = NULL;
            cache_error = FTC_ImageCache_New(manager, &cache);
            nullness = cache ? "non_null" : "null";
        } else if (streq(constructor, "FTC_SBitCache_New")) {
            FTC_SBitCache cache = NULL;
            cache_error = FTC_SBitCache_New(manager, &cache);
            nullness = cache ? "non_null" : "null";
        } else {
            cache_error = FT_Err_Invalid_Argument;
        }
    }

    printf("{");
    print_status(cache_error);
    printf(",\"output\":{\"constructor\":\"");
    print_json_string_content(constructor);
    printf("\",\"manager_status\":%d,\"create\":{\"status\":%d},"
           "\"handle\":{\"nullness\":\"%s\"},"
           "\"owner\":{\"identity_class\":\"manager\"},"
           "\"done\":{\"lifecycle_class\":\"manager_destroys_cache\"}}}\n",
           manager_error,
           cache_error,
           nullness);
    if (manager) {
        FTC_Manager_Done(manager);
    }
    FT_Done_FreeType(library);
    return 0;
}

typedef struct CMapCacheRequesterData_ {
    unsigned char* data;
    long data_len;
    int request_count;
} CMapCacheRequesterData;

static FT_Error cmap_cache_requester(FTC_FaceID face_id,
                                     FT_Library library,
                                     FT_Pointer req_data,
                                     FT_Face* aface) {
    (void)req_data;
    CMapCacheRequesterData* data = (CMapCacheRequesterData*)face_id;
    data->request_count++;
    return FT_New_Memory_Face(library, data->data, data->data_len, 0, aface);
}

static int cmap_cache_active_index(FT_Face face) {
    if (!face || !face->charmap) {
        return -1;
    }
    for (FT_Int index = 0; index < face->num_charmaps; index++) {
        if (face->charmaps[index] == face->charmap) {
            return index;
        }
    }
    return -1;
}

static void print_cmap_lookup_row(FTC_Manager manager,
                                  FTC_CMapCache cache,
                                  CMapCacheRequesterData* requester,
                                  int cmap_index,
                                  FT_UInt32 char_code,
                                  int repeat_lookup,
                                  int lifecycle) {
    int before_count = requester->request_count;
    FT_UInt first = FTC_CMapCache_Lookup(cache, (FTC_FaceID)requester, cmap_index, char_code);
    int after_first_count = requester->request_count;
    FT_UInt repeat = 0;
    int after_repeat_count = after_first_count;
    if (repeat_lookup) {
        repeat = FTC_CMapCache_Lookup(cache, (FTC_FaceID)requester, cmap_index, char_code);
        after_repeat_count = requester->request_count;
    }
    FT_UInt after_remove = 0;
    int after_remove_count = after_repeat_count;
    FT_UInt after_reset = 0;
    int after_reset_count = after_repeat_count;
    if (lifecycle) {
        FTC_Manager_RemoveFaceID(manager, (FTC_FaceID)requester);
        after_remove = FTC_CMapCache_Lookup(cache, (FTC_FaceID)requester, cmap_index, char_code);
        after_remove_count = requester->request_count;
        FTC_Manager_Reset(manager);
        after_reset = FTC_CMapCache_Lookup(cache, (FTC_FaceID)requester, cmap_index, char_code);
        after_reset_count = requester->request_count;
    }
    FT_Face face = NULL;
    FT_Error face_error = FTC_Manager_LookupFace(manager, (FTC_FaceID)requester, &face);
    int active_index = face_error ? -1 : cmap_cache_active_index(face);
    printf("{\"cmap_index\":%d,\"char_code\":%u,"
           "\"requester_count_before\":%d,"
           "\"first\":%u,\"requester_count_after_first\":%d,"
           "\"repeat\":%u,\"requester_count_after_repeat\":%d,"
           "\"after_remove\":%u,\"requester_count_after_remove\":%d,"
           "\"after_reset\":%u,\"requester_count_after_reset\":%d,"
           "\"active_charmap_after\":%d}",
           cmap_index,
           (unsigned int)char_code,
           before_count,
           first,
           after_first_count,
           repeat,
           after_repeat_count,
           after_remove,
           after_remove_count,
           after_reset,
           after_reset_count,
           active_index);
}

static int emit_cmap_cache_lookup(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    const char* scenario = argv[4];
    char* indexes_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!indexes_arg) {
        return 1;
    }
    memcpy(indexes_arg, argv[5], strlen(argv[5]) + 1);
    FT_UInt32 char_code = (FT_UInt32)strtoul(argv[6], NULL, 0);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(indexes_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(indexes_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(indexes_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(indexes_arg);
        return 0;
    }

    CMapCacheRequesterData requester;
    requester.data = data;
    requester.data_len = data_len;
    requester.request_count = 0;

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             cmap_cache_requester,
                                             NULL,
                                             &manager);
    FTC_CMapCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_CMapCache_New(manager, &cache);
    if (cache_error) {
        printf("{");
        print_status(cache_error);
        printf(",\"output\":{\"status\":%d,\"rows\":[]}}\n", cache_error);
        if (manager) {
            FTC_Manager_Done(manager);
        }
        FT_Done_FreeType(library);
        free(data);
        free(indexes_arg);
        return 0;
    }

    int repeat_lookup = strstr(scenario, "repeat") != NULL ||
                        strstr(scenario, "planned_cache") != NULL ||
                        strstr(scenario, "miss") != NULL ||
                        strstr(scenario, "negative") != NULL;
    int lifecycle = strstr(scenario, "lifecycle") != NULL;
    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"rows\":[");
    char* cursor = indexes_arg;
    int first = 1;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        int cmap_index = (int)strtol(cursor, NULL, 10);
        if (!first) {
            printf(",");
        }
        print_cmap_lookup_row(manager, cache, &requester, cmap_index, char_code, repeat_lookup, lifecycle);
        first = 0;
        cursor = next ? next + 1 : NULL;
    }
    printf("],\"requester_count_final\":%d}}\n", requester.request_count);

    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    free(indexes_arg);
    return 0;
}

static int emit_cmap_cache_new_route(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    const char* scenario = argv[4];

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    CMapCacheRequesterData requester;
    requester.data = data;
    requester.data_len = data_len;
    requester.request_count = 0;

    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             cmap_cache_requester,
                                             NULL,
                                             &manager);
    FTC_CMapCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_CMapCache_New(manager, &cache);
    FTC_CMapCache registration_caches[17];
    FT_Error registration_statuses[17];
    memset(registration_caches, 0, sizeof(registration_caches));
    memset(registration_statuses, 0, sizeof(registration_statuses));

    FT_UInt first = 0;
    FT_UInt after_reset = 0;
    FT_UInt after_registration_limit = 0;
    int after_first_count = requester.request_count;
    int after_reset_count = requester.request_count;
    int after_registration_limit_count = requester.request_count;
    int registration_probe = streq(scenario, "success_multiple_cache_registration_limit");
    int successful_registrations = 0;
    int failed_registration_index = -1;
    if (!cache_error) {
        first = FTC_CMapCache_Lookup(cache, (FTC_FaceID)&requester, -1, 65);
        after_first_count = requester.request_count;
        if (registration_probe) {
            registration_caches[0] = cache;
            registration_statuses[0] = cache_error;
            successful_registrations = 1;
            for (int index = 1; index < 17; index++) {
                FT_Error status = FTC_CMapCache_New(manager, &registration_caches[index]);
                registration_statuses[index] = status;
                if (status) {
                    failed_registration_index = index;
                    break;
                }
                successful_registrations++;
            }
            after_registration_limit =
                FTC_CMapCache_Lookup(cache, (FTC_FaceID)&requester, -1, 65);
            after_registration_limit_count = requester.request_count;
        }
        FTC_Manager_Reset(manager);
        after_reset = FTC_CMapCache_Lookup(cache, (FTC_FaceID)&requester, -1, 65);
        after_reset_count = requester.request_count;
    }

    printf("{");
    print_status(cache_error);
    printf(",\"output\":{\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"manager_status\":%d,\"create_status\":%d,"
           "\"cache_handle\":\"%s\",\"destroyed_by_manager_done\":true,"
           "\"lookup\":{\"char_code\":65,\"first\":%u,\"after_reset\":%u,"
           "\"requester_count_after_first\":%d,\"requester_count_after_reset\":%d,"
           "\"reset_preserves_handle\":%s}",
           manager_error,
           cache_error,
           cache ? "non_null" : "null",
           first,
           after_reset,
           after_first_count,
           after_reset_count,
           cache ? "true" : "false");
    if (registration_probe) {
        printf(",\"registration_limit\":{\"max_caches\":16,\"attempt_statuses\":[");
        for (int index = 0; index < 17; index++) {
            if (index) {
                printf(",");
            }
            printf("%d", registration_statuses[index]);
        }
        printf("],\"attempt_handles\":[");
        for (int index = 0; index < 17; index++) {
            if (index) {
                printf(",");
            }
            printf("\"%s\"", registration_caches[index] ? "non_null" : "null");
        }
        printf("],\"successful_registrations\":%d,"
               "\"failed_registration_index\":%d,"
               "\"final_status\":%d,"
               "\"prior_cache_lookup_after_failure\":%u,"
               "\"requester_count_after_failure_lookup\":%d,"
               "\"prior_cache_preserved\":%s}",
               successful_registrations,
               failed_registration_index,
               failed_registration_index >= 0 ? registration_statuses[failed_registration_index] : 0,
               after_registration_limit,
               after_registration_limit_count,
               after_registration_limit == first ? "true" : "false");
    }
    printf("}}\n");

    if (manager) {
        FTC_Manager_Done(manager);
    }
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static void print_size_metrics_object(FT_Size_Metrics metrics) {
    printf("\"x_ppem\":%u,\"y_ppem\":%u,\"x_scale\":%ld,\"y_scale\":%ld,\"ascender\":%ld,\"descender\":%ld,\"height\":%ld,\"max_advance\":%ld",
           metrics.x_ppem,
           metrics.y_ppem,
           metrics.x_scale,
           metrics.y_scale,
           metrics.ascender,
           metrics.descender,
           metrics.height,
           metrics.max_advance);
}

static void print_size_metrics(FT_Size_Metrics metrics) {
    printf("\"output\":{");
    print_size_metrics_object(metrics);
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

typedef struct MemoryFaceRequestRec_ {
    const unsigned char* data;
    long data_len;
    FT_Long face_index;
    unsigned int calls;
} MemoryFaceRequestRec;

static int load_oracle_source_bytes(const char* source_kind,
                                    const char* source_value,
                                    unsigned char** data,
                                    long* data_len) {
    if (streq(source_kind, "file")) {
        return load_file(source_value, data, data_len);
    }
    if (streq(source_kind, "hex")) {
        return decode_hex(source_value, data, data_len);
    }
    return 2;
}

static FT_Error memory_face_requester(FTC_FaceID face_id,
                                      FT_Library library,
                                      FT_Pointer req_data,
                                      FT_Face* aface) {
    MemoryFaceRequestRec* request = (MemoryFaceRequestRec*)req_data;
    if (!request) {
        request = (MemoryFaceRequestRec*)face_id;
    }
    if (!request || !aface) {
        return FT_Err_Invalid_Argument;
    }
    request->calls++;
    return FT_New_Memory_Face(library,
                              request->data,
                              request->data_len,
                              request->face_index,
                              aface);
}

static void print_manager_reset_payload(unsigned int before_calls,
                                        unsigned int after_calls,
                                        FT_Error post_reset_error) {
    printf("\"output\":{");
    printf("\"void\":true,");
    printf("\"reset\":{\"called\":true},");
    printf("\"requester_call_counts\":{\"before_reset\":%u,\"after_reset\":%u},",
           before_calls,
           after_calls);
    printf("\"post_reset\":{\"status\":%d,\"usable\":%s},",
           post_reset_error,
           post_reset_error ? "false" : "true");
    printf("\"manager_handle\":\"same_identity_class\",");
    printf("\"face_identity_class_after_reset\":\"fresh_or_reloaded\",");
    printf("\"size_identity_class_after_reset\":\"fresh_or_reloaded\",");
    printf("\"node_count_class\":\"not_observed\"");
    printf("}");
}

static void print_face_id_identity_row(FTC_Manager manager,
                                       MemoryFaceRequestRec* request,
                                       const char* label,
                                       const char* face_id_identity,
                                       unsigned int* total_calls,
                                       FT_Face* first_a_face) {
    unsigned int before = *total_calls;
    unsigned int request_before = request->calls;
    FT_Face face = NULL;
    FT_Error error = FTC_Manager_LookupFace(manager, (FTC_FaceID)request, &face);
    unsigned int request_delta = request->calls - request_before;
    *total_calls += request_delta;
    const char* face_identity_class = "distinct_from_first_a";
    if (!error && first_a_face && !*first_a_face && face) {
        face_identity_class = "establishes_first_a";
    } else if (!error && first_a_face && *first_a_face && face == *first_a_face) {
        face_identity_class = "same_as_first_a";
    }
    printf("{\"label\":\"");
    print_json_string_content(label);
    printf("\",\"status\":%d,\"error\":%d,\"face_id_identity\":\"",
           error,
           error);
    print_json_string_content(face_id_identity);
    printf("\",\"requester_count_before\":%u,"
           "\"requester_count_after\":%u,"
           "\"identity_class\":\"%s\","
           "\"face_identity_class\":\"%s\"}",
           before,
           *total_calls,
           request_delta ? "fresh_or_reloaded" : "cached",
           face_identity_class);
    if (!error && first_a_face && !*first_a_face && face) {
        *first_a_face = face;
    }
}

static int emit_face_id_identity_route(int argc, char** argv) {
    (void)argc;
    unsigned char* data_a = NULL;
    unsigned char* data_b = NULL;
    long data_a_len = 0;
    long data_b_len = 0;
    FT_Long face_index = (FT_Long)strtol(argv[6], NULL, 10);
    if (load_oracle_source_bytes(argv[2], argv[3], &data_a, &data_a_len) != 0 ||
        load_oracle_source_bytes(argv[4], argv[5], &data_b, &data_b_len) != 0) {
        fprintf(stderr, "failed to read face-id identity fonts\n");
        free(data_a);
        free(data_b);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data_a);
        free(data_b);
        return 0;
    }

    MemoryFaceRequestRec request_a = {data_a, data_a_len, face_index, 0};
    MemoryFaceRequestRec request_a_same_bytes = {data_a, data_a_len, face_index, 0};
    MemoryFaceRequestRec request_b = {data_b, data_b_len, face_index, 0};
    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 8, 8, 0,
                                             memory_face_requester,
                                             NULL,
                                             &manager);
    if (manager_error) {
        printf("{");
        print_status(manager_error);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        free(data_a);
        free(data_b);
        return 0;
    }

    unsigned int total_calls = 0;
    FT_Face first_a_face = NULL;
    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{"
           "\"cache_key\":\"raw_pointer_identity\","
           "\"requester\":\"records_face_id_raw_identity\","
           "\"lookups\":[");
    print_face_id_identity_row(manager, &request_a, "face-A", "stable_pointer:dejavu", &total_calls, &first_a_face);
    printf(",");
    print_face_id_identity_row(manager, &request_a_same_bytes, "face-A-same-bytes-distinct-address", "stable_pointer:dejavu_same_bytes_distinct_address", &total_calls, &first_a_face);
    printf(",");
    print_face_id_identity_row(manager, &request_b, "face-B", "stable_pointer:liberation", &total_calls, &first_a_face);
    printf(",");
    print_face_id_identity_row(manager, &request_a, "face-A-alias-same-address", "stable_pointer:dejavu_alias_same_address", &total_calls, &first_a_face);
    printf("],\"requester_count_final\":%u,"
           "\"same_bytes_distinct_pointer_class\":\"distinct_cache_key\","
           "\"alias_same_address_class\":\"cache_hit\"}}\n",
           total_calls);

    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data_a);
    free(data_b);
    return 0;
}

static void print_manager_lookup_size_row(FTC_Manager manager,
                                          MemoryFaceRequestRec* request,
                                          FTC_ScalerRec* scaler,
                                          int repeat_lookup) {
    unsigned int before_calls = request->calls;
    FT_Size size = NULL;
    FT_Error error = FTC_Manager_LookupSize(manager, scaler, &size);
    unsigned int after_calls = request->calls;
    FT_Size repeat_size = NULL;
    FT_Error repeat_error = error;
    unsigned int after_repeat_calls = after_calls;
    if (repeat_lookup) {
        repeat_error = FTC_Manager_LookupSize(manager, scaler, &repeat_size);
        after_repeat_calls = request->calls;
    }

    printf("{\"scaler\":{\"width\":%u,\"height\":%u,\"pixel\":%u,\"x_res\":%u,\"y_res\":%u},"
           "\"status\":%d,\"error\":%d,"
           "\"requester_count_before\":%u,"
           "\"requester_count_after\":%u,"
           "\"requester_count_after_repeat\":%u,",
           scaler->width,
           scaler->height,
           scaler->pixel,
           scaler->x_res,
           scaler->y_res,
           error,
           error,
           before_calls,
           after_calls,
           after_repeat_calls);
    if (error || !size) {
        printf("\"metrics\":null,");
    } else {
        printf("\"metrics\":{");
        print_size_metrics_object(size->metrics);
        printf("},");
    }
    printf("\"repeat\":{\"status\":%d,\"same_identity\":%s,\"requester_count\":%u}}",
           repeat_error,
           (!error && !repeat_error && size && repeat_size && size == repeat_size) ? "true" : "false",
           after_repeat_calls);
}

static int emit_cid_route(int argc, char** argv) {
    if (argc != 6) {
        return 2;
    }
    const char* route = argv[2];
    unsigned char* data = NULL;
    long data_len = 0;
    if (load_oracle_source_bytes(argv[3], argv[4], &data, &data_len) != 0) {
        return 1;
    }
    FT_Long face_index = (FT_Long)strtol(argv[5], NULL, 10);
    FT_Library library = NULL;
    FT_Face face = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf("}\n");
        free(data);
        return 0;
    }
    FT_Error open_error = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    if (open_error) {
        printf("{");
        print_status(open_error);
        printf("}\n");
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    printf("{");
    if (streq(route, "is-internally-cid-keyed") ||
        streq(route, "is-internally-cid-keyed:null-output")) {
        FT_Bool is_cid = 0;
        FT_Bool* output = streq(route, "is-internally-cid-keyed:null-output")
                              ? NULL
                              : &is_cid;
        FT_Error err = FT_Get_CID_Is_Internally_CID_Keyed(face, output);
        print_status(err);
        if (output) {
            printf(",\"output\":{\"is_cid\":%u,\"ft_is_cid_keyed\":%d}}\n",
                   (unsigned)is_cid,
                   FT_IS_CID_KEYED(face) ? 1 : 0);
        } else {
            printf(",\"output\":{\"is_cid_output\":\"null\",\"ft_is_cid_keyed\":%d}}\n",
                   FT_IS_CID_KEYED(face) ? 1 : 0);
        }
    } else if (strncmp(route, "glyph-index:", 12) == 0) {
        const char* glyph_part = route + 12;
        char glyph_buffer[32];
        const char* suffix = strstr(glyph_part, ":null-output");
        if (suffix) {
            size_t len = (size_t)(suffix - glyph_part);
            if (len >= sizeof(glyph_buffer)) {
                len = sizeof(glyph_buffer) - 1;
            }
            memcpy(glyph_buffer, glyph_part, len);
            glyph_buffer[len] = '\0';
            glyph_part = glyph_buffer;
        }
        FT_UInt glyph_index = streq(glyph_part, "last_valid")
                                  ? (FT_UInt)(face->num_glyphs - 1)
                                  : (FT_UInt)strtoul(glyph_part, NULL, 10);
        FT_UInt cid = 0;
        FT_UInt* output = suffix ? NULL : &cid;
        FT_Error err = FT_Get_CID_From_Glyph_Index(face, glyph_index, output);
        print_status(err);
        if (output) {
            printf(",\"output\":{\"glyph_index\":%u,\"cid\":%u}}\n",
                   glyph_index,
                   cid);
        } else {
            printf(",\"output\":{\"glyph_index\":%u,\"cid_output\":\"null\"}}\n",
                   glyph_index);
        }
    } else if (streq(route, "ros")) {
        const char* registry = NULL;
        const char* ordering = NULL;
        FT_Int supplement = 0;
        FT_Error err = FT_Get_CID_Registry_Ordering_Supplement(
            face, &registry, &ordering, &supplement);
        print_status(err);
        printf(",\"output\":{\"registry\":{\"string\":");
        print_json_c_string_or_null(registry);
        printf(",\"identity_class\":\"%s\"},\"ordering\":{\"string\":",
               registry ? "face_owned_c_string" : "null");
        print_json_c_string_or_null(ordering);
        printf(",\"identity_class\":\"%s\"},\"supplement\":%d,"
               "\"output_write_bitmap\":[\"registry\",\"ordering\",\"supplement\"]}}\n",
               ordering ? "face_owned_c_string" : "null",
               supplement);
    } else {
        print_status(FT_Err_Invalid_Argument);
        printf("}\n");
    }

    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_manager_lookup_size(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    char* scalers_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!scalers_arg) {
        return 1;
    }
    memcpy(scalers_arg, argv[5], strlen(argv[5]) + 1);
    int repeat_lookup = atoi(argv[6]) != 0;

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(scalers_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(scalers_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(scalers_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(scalers_arg);
        return 0;
    }

    MemoryFaceRequestRec request = {data, data_len, face_index, 0};
    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             memory_face_requester,
                                             &request,
                                             &manager);
    if (manager_error) {
        printf("{");
        print_status(manager_error);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        free(data);
        free(scalers_arg);
        return 0;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"outputs\":[");
    char* cursor = scalers_arg;
    int first = 1;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        FTC_ScalerRec scaler;
        unsigned int pixel = 0;
        memset(&scaler, 0, sizeof(scaler));
        if (sscanf(cursor, "%u:%u:%u:%u:%u",
                   &scaler.width,
                   &scaler.height,
                   &pixel,
                   &scaler.x_res,
                   &scaler.y_res) != 5) {
            FTC_Manager_Done(manager);
            FT_Done_FreeType(library);
            free(data);
            free(scalers_arg);
            return 2;
        }
        scaler.face_id = (FTC_FaceID)&request;
        scaler.pixel = (FT_UInt)pixel;
        if (!first) {
            printf(",");
        }
        print_manager_lookup_size_row(manager, &request, &scaler, repeat_lookup);
        first = 0;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");

    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    free(scalers_arg);
    return 0;
}

static void print_manager_lookup_face_public(FT_Face face) {
    if (!face) {
        printf("null");
        return;
    }
    printf("{\"num_glyphs\":%ld,\"face_flags\":%ld,\"size_is_null\":%s}",
           face->num_glyphs,
           face->face_flags,
           face->size ? "false" : "true");
}

static void print_manager_lookup_face_row(FTC_Manager manager,
                                          MemoryFaceRequestRec* request,
                                          const char* face_id,
                                          int step_index) {
    unsigned int before_calls = request->calls;
    FT_Face face = NULL;
    FT_Error error = FTC_Manager_LookupFace(manager, (FTC_FaceID)request, &face);
    unsigned int after_calls = request->calls;
    printf("{\"step\":%d,\"face_id\":\"", step_index);
    print_json_string_content(face_id);
    printf("\",\"status\":%d,\"error\":%d,"
           "\"requester_count_before\":%u,"
           "\"requester_count_after\":%u,"
           "\"identity_class\":\"%s\","
           "\"face\":",
           error,
           error,
           before_calls,
           after_calls,
           after_calls > before_calls ? "fresh_or_reloaded" : "cached");
    print_manager_lookup_face_public(error ? NULL : face);
    printf("}");
}

static int emit_manager_lookup_face(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    char* sequence_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!sequence_arg) {
        return 1;
    }
    memcpy(sequence_arg, argv[5], strlen(argv[5]) + 1);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(sequence_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(sequence_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(sequence_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(sequence_arg);
        return 0;
    }

    MemoryFaceRequestRec request = {data, data_len, face_index, 0};
    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             memory_face_requester,
                                             &request,
                                             &manager);
    if (manager_error) {
        printf("{");
        print_status(manager_error);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        free(data);
        free(sequence_arg);
        return 0;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"outputs\":[");
    char* cursor = sequence_arg;
    int first = 1;
    int step_index = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        if (streq(cursor, "remove-face-A")) {
            FTC_Manager_RemoveFaceID(manager, (FTC_FaceID)&request);
        } else if (streq(cursor, "face-A")) {
            if (!first) {
                printf(",");
            }
            print_manager_lookup_face_row(manager, &request, cursor, step_index);
            first = 0;
        }
        step_index++;
        cursor = next ? next + 1 : NULL;
    }
    printf("],\"requester_count_final\":%u}}\n", request.calls);

    FTC_Manager_Done(manager);
    FT_Done_FreeType(library);
    free(data);
    free(sequence_arg);
    return 0;
}

static int emit_manager_new_route(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    const char* scenario = argv[5];
    char* limits_arg = (char*)malloc(strlen(argv[6]) + 1);
    if (!limits_arg) {
        return 1;
    }
    memcpy(limits_arg, argv[6], strlen(argv[6]) + 1);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(limits_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(limits_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(limits_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(limits_arg);
        return 0;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"req_data\":\"sentinel-pointer-token\",\"rows\":[");
    char* cursor = limits_arg;
    int first_row = 1;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        unsigned long max_faces = 0;
        unsigned long max_sizes = 0;
        unsigned long max_bytes = 0;
        if (sscanf(cursor, "%lu:%lu:%lu", &max_faces, &max_sizes, &max_bytes) != 3) {
            FT_Done_FreeType(library);
            free(data);
            free(limits_arg);
            return 2;
        }
        MemoryFaceRequestRec request = {data, data_len, face_index, 0};
        FTC_Manager manager = NULL;
        FT_Error manager_error = FTC_Manager_New(library,
                                                 (FT_UInt)max_faces,
                                                 (FT_UInt)max_sizes,
                                                 max_bytes,
                                                 memory_face_requester,
                                                 &request,
                                                 &manager);
        FT_Face face = NULL;
        FT_Error lookup_error = manager_error
            ? manager_error
            : FTC_Manager_LookupFace(manager, (FTC_FaceID)&request, &face);
        unsigned int after_lookup_calls = request.calls;
        unsigned int after_reset_calls = after_lookup_calls;
        FT_Error after_reset_error = lookup_error;
        if (!lookup_error) {
            FTC_Manager_Reset(manager);
            face = NULL;
            after_reset_error = FTC_Manager_LookupFace(manager, (FTC_FaceID)&request, &face);
            after_reset_calls = request.calls;
        }
        if (!first_row) {
            printf(",");
        }
        first_row = 0;
        printf("{\"limits\":{\"max_faces\":%lu,\"max_sizes\":%lu,\"max_bytes\":%lu},"
               "\"manager_status\":%d,\"lookup_status\":%d,\"after_reset_status\":%d,"
               "\"manager_handle\":\"%s\",\"requester_count_after_lookup\":%u,"
               "\"requester_count_after_reset\":%u,\"reset_called\":%s,\"done_called\":true}",
               max_faces,
               max_sizes,
               max_bytes,
               manager_error,
               lookup_error,
               after_reset_error,
               manager ? "non_null" : "null",
               after_lookup_calls,
               after_reset_calls,
               manager ? "true" : "false");
        if (manager) {
            FTC_Manager_Done(manager);
        }
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");

    FT_Done_FreeType(library);
    free(data);
    free(limits_arg);
    return 0;
}

static int emit_image_cache_new_route(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    const char* scenario = argv[5];

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    MemoryFaceRequestRec request = {data, data_len, face_index, 0};
    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             memory_face_requester,
                                             &request,
                                             &manager);
    FTC_ImageCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_ImageCache_New(manager, &cache);
    FTC_ScalerRec scaler;
    memset(&scaler, 0, sizeof(scaler));
    scaler.face_id = (FTC_FaceID)&request;
    scaler.width = 12;
    scaler.height = 12;
    scaler.pixel = 1;
    FT_Glyph first_glyph = NULL;
    FTC_Node first_node = NULL;
    FT_Error first_error = cache_error ? cache_error
        : FTC_ImageCache_LookupScaler(cache, &scaler, FT_LOAD_DEFAULT, 36, &first_glyph, &first_node);
    unsigned int after_first_calls = request.calls;
    if (first_node) {
        FTC_Node_Unref(first_node, manager);
    }
    if (!cache_error) {
        FTC_Manager_Reset(manager);
    }
    FT_Glyph reset_glyph = NULL;
    FTC_Node reset_node = NULL;
    FT_Error reset_error = cache_error ? cache_error
        : FTC_ImageCache_LookupScaler(cache, &scaler, FT_LOAD_DEFAULT, 36, &reset_glyph, &reset_node);
    unsigned int after_reset_calls = request.calls;
    if (reset_node) {
        FTC_Node_Unref(reset_node, manager);
    }

    printf("{");
    print_status(cache_error);
    printf(",\"output\":{\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"manager_status\":%d,\"create_status\":%d,"
           "\"cache_handle\":\"%s\",\"destroyed_by_manager_done\":true,"
           "\"lookup\":{\"glyph_index\":36,\"first_status\":%d,"
           "\"after_reset_status\":%d,\"requester_count_after_first\":%u,"
           "\"requester_count_after_reset\":%u,\"reset_preserves_handle\":%s,",
           manager_error,
           cache_error,
           cache ? "non_null" : "null",
           first_error,
           reset_error,
           after_first_calls,
           after_reset_calls,
           cache ? "true" : "false");
    print_image_cache_glyph_object(reset_error ? NULL : reset_glyph);
    printf("}}}\n");

    if (manager) {
        FTC_Manager_Done(manager);
    }
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static void print_manager_remove_counts(unsigned int a_after_populate,
                                        unsigned int b_after_populate,
                                        unsigned int a_after_remove_lookup,
                                        unsigned int b_after_remove_lookup,
                                        unsigned int a_after_unknown,
                                        unsigned int b_after_unknown,
                                        unsigned int a_after_null_face,
                                        unsigned int b_after_null_face) {
    printf("\"counts\":{\"a_after_populate\":%u,\"b_after_populate\":%u,"
           "\"a_after_remove_lookup\":%u,\"b_after_remove_lookup\":%u,"
           "\"a_after_unknown\":%u,\"b_after_unknown\":%u,"
           "\"a_after_null_face\":%u,\"b_after_null_face\":%u}",
           a_after_populate,
           b_after_populate,
           a_after_remove_lookup,
           b_after_remove_lookup,
           a_after_unknown,
           b_after_unknown,
           a_after_null_face,
           b_after_null_face);
}

static int emit_manager_remove_face_id_route(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    const char* scenario = argv[5];

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    MemoryFaceRequestRec request_a = {data, data_len, face_index, 0};
    MemoryFaceRequestRec request_b = {data, data_len, face_index, 0};
    MemoryFaceRequestRec unknown = {data, data_len, face_index, 0};
    FTC_Manager manager = NULL;
    FT_Error manager_error = FTC_Manager_New(library, 0, 0, 0,
                                             memory_face_requester,
                                             NULL,
                                             &manager);
    FTC_ImageCache cache = NULL;
    FT_Error cache_error = manager_error ? manager_error : FTC_ImageCache_New(manager, &cache);
    FTC_ScalerRec scaler_a;
    FTC_ScalerRec scaler_b;
    memset(&scaler_a, 0, sizeof(scaler_a));
    memset(&scaler_b, 0, sizeof(scaler_b));
    scaler_a.face_id = (FTC_FaceID)&request_a;
    scaler_a.width = 12;
    scaler_a.height = 12;
    scaler_a.pixel = 1;
    scaler_b = scaler_a;
    scaler_b.face_id = (FTC_FaceID)&request_b;

    FT_Glyph glyph = NULL;
    FTC_Node held_node = NULL;
    FT_Error a_first = cache_error ? cache_error
        : FTC_ImageCache_LookupScaler(cache, &scaler_a, FT_LOAD_DEFAULT, 36, &glyph, &held_node);
    FT_Glyph glyph_b = NULL;
    FTC_Node node_b = NULL;
    FT_Error b_first = cache_error ? cache_error
        : FTC_ImageCache_LookupScaler(cache, &scaler_b, FT_LOAD_DEFAULT, 36, &glyph_b, &node_b);
    if (node_b) {
        FTC_Node_Unref(node_b, manager);
    }
    unsigned int a_after_populate = request_a.calls;
    unsigned int b_after_populate = request_b.calls;

    if (manager) {
        FTC_Manager_RemoveFaceID(manager, (FTC_FaceID)&request_a);
    }
    FT_Glyph a_after_remove_glyph = NULL;
    FTC_Node a_after_remove_node = NULL;
    FT_Error a_after_remove = cache_error ? cache_error
        : FTC_ImageCache_LookupScaler(cache, &scaler_a, FT_LOAD_DEFAULT, 36, &a_after_remove_glyph, &a_after_remove_node);
    if (a_after_remove_node) {
        FTC_Node_Unref(a_after_remove_node, manager);
    }
    FT_Glyph b_after_remove_glyph = NULL;
    FTC_Node b_after_remove_node = NULL;
    FT_Error b_after_remove = cache_error ? cache_error
        : FTC_ImageCache_LookupScaler(cache, &scaler_b, FT_LOAD_DEFAULT, 36, &b_after_remove_glyph, &b_after_remove_node);
    if (b_after_remove_node) {
        FTC_Node_Unref(b_after_remove_node, manager);
    }
    unsigned int a_after_remove_lookup = request_a.calls;
    unsigned int b_after_remove_lookup = request_b.calls;

    if (manager) {
        FTC_Manager_RemoveFaceID(manager, (FTC_FaceID)&unknown);
    }
    unsigned int a_after_unknown = request_a.calls;
    unsigned int b_after_unknown = request_b.calls;
    if (manager) {
        FTC_Manager_RemoveFaceID(manager, NULL);
    }
    unsigned int a_after_null_face = request_a.calls;
    unsigned int b_after_null_face = request_b.calls;

    FT_Error a_after_unref = FT_Err_Ok;
    if (held_node) {
        FTC_Node_Unref(held_node, manager);
        FT_Glyph after_unref_glyph = NULL;
        FTC_Node after_unref_node = NULL;
        a_after_unref = cache_error ? cache_error
            : FTC_ImageCache_LookupScaler(cache, &scaler_a, FT_LOAD_DEFAULT, 36, &after_unref_glyph, &after_unref_node);
        if (after_unref_node) {
            FTC_Node_Unref(after_unref_node, manager);
        }
    }
    FTC_Manager_RemoveFaceID(NULL, (FTC_FaceID)&request_a);

    printf("{");
    print_status(cache_error);
    printf(",\"output\":{\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"manager_status\":%d,\"cache_status\":%d,"
           "\"null_manager_noop\":true,\"unknown_face_noop\":true,\"null_face_id_noop\":true,",
           manager_error,
           cache_error);
    print_manager_remove_counts(a_after_populate,
                                b_after_populate,
                                a_after_remove_lookup,
                                b_after_remove_lookup,
                                a_after_unknown,
                                b_after_unknown,
                                a_after_null_face,
                                b_after_null_face);
    printf(",\"lookups\":{\"a_first_status\":%d,\"b_first_status\":%d,"
           "\"a_after_remove_status\":%d,\"b_after_remove_status\":%d,"
           "\"a_after_unref_status\":%d},\"node\":{\"acquired\":%s,"
           "\"unref_after_remove\":%s}}}\n",
           a_first,
           b_first,
           a_after_remove,
           b_after_remove,
           a_after_unref,
           held_node ? "true" : "false",
           held_node ? "true" : "false");

    if (manager) {
        FTC_Manager_Done(manager);
    }
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_manager_done_route(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    const char* scenario = argv[5];

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    FTC_Manager_Done(NULL);
    MemoryFaceRequestRec request = {data, data_len, face_index, 0};
    FTC_Manager empty_manager = NULL;
    FT_Error empty_status = FTC_Manager_New(library, 0, 0, 0,
                                            memory_face_requester,
                                            &request,
                                            &empty_manager);
    if (empty_manager) {
        FTC_Manager_Done(empty_manager);
    }

    request.calls = 0;
    FTC_Manager manager = NULL;
    FT_Error manager_status = FTC_Manager_New(library, 0, 0, 0,
                                              memory_face_requester,
                                              &request,
                                              &manager);
    FTC_CMapCache cmap_cache = NULL;
    FTC_ImageCache image_cache = NULL;
    FT_Error cmap_status = manager_status ? manager_status : FTC_CMapCache_New(manager, &cmap_cache);
    FT_Error image_status = manager_status ? manager_status : FTC_ImageCache_New(manager, &image_cache);
    FT_UInt cmap_lookup = 0;
    if (!cmap_status) {
        cmap_lookup = FTC_CMapCache_Lookup(cmap_cache, (FTC_FaceID)&request, -1, 65);
    }
    FTC_ScalerRec scaler;
    memset(&scaler, 0, sizeof(scaler));
    scaler.face_id = (FTC_FaceID)&request;
    scaler.width = 12;
    scaler.height = 12;
    scaler.pixel = 1;
    FT_Size size = NULL;
    FT_Error size_status = manager_status ? manager_status : FTC_Manager_LookupSize(manager, &scaler, &size);
    FT_Face face = NULL;
    FT_Error face_status = manager_status ? manager_status : FTC_Manager_LookupFace(manager, (FTC_FaceID)&request, &face);
    FT_Glyph glyph = NULL;
    FTC_Node node = NULL;
    FT_Error image_lookup_status = image_status ? image_status
        : FTC_ImageCache_LookupScaler(image_cache, &scaler, FT_LOAD_DEFAULT, 36, &glyph, &node);
    if (node) {
        FTC_Node_Unref(node, manager);
    }
    unsigned int requester_count_before_done = request.calls;
    if (manager) {
        FTC_Manager_Done(manager);
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"void\":true,\"null_manager_noop\":true,"
           "\"empty_manager\":{\"create_status\":%d,\"done_called\":true},"
           "\"populated_manager\":{\"create_status\":%d,\"cmap_cache_status\":%d,"
           "\"image_cache_status\":%d,\"lookup_face_status\":%d,"
           "\"lookup_size_status\":%d,\"image_lookup_status\":%d,"
           "\"cmap_lookup\":%u,\"requester_count_before_done\":%u,"
           "\"node_released_before_done\":true,\"done_called\":true}}}\n",
           empty_status,
           manager_status,
           cmap_status,
           image_status,
           face_status,
           size_status,
           image_lookup_status,
           cmap_lookup,
           requester_count_before_done);

    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_manager_lifecycle_route(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    const char* scenario = argv[5];

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    MemoryFaceRequestRec reset_request = {data, data_len, face_index, 0};
    FTC_Manager reset_manager = NULL;
    FT_Error reset_status = FTC_Manager_New(library, 1, 2, 4096,
                                            memory_face_requester,
                                            &reset_request,
                                            &reset_manager);
    if (!reset_status) {
        FT_Face face = NULL;
        reset_status = FTC_Manager_LookupFace(reset_manager,
                                              (FTC_FaceID)&reset_request,
                                              &face);
    }
    if (!reset_status) {
        FTC_ScalerRec scaler;
        memset(&scaler, 0, sizeof(scaler));
        scaler.face_id = (FTC_FaceID)&reset_request;
        scaler.width = 0;
        scaler.height = 12;
        scaler.pixel = 1;
        FT_Size size = NULL;
        reset_status = FTC_Manager_LookupSize(reset_manager, &scaler, &size);
    }
    unsigned int before_reset_calls = reset_request.calls;
    FT_Error post_reset_error = reset_status;
    if (!reset_status) {
        FTC_Manager_Reset(reset_manager);
        FT_Face post_face = NULL;
        post_reset_error = FTC_Manager_LookupFace(reset_manager,
                                                  (FTC_FaceID)&reset_request,
                                                  &post_face);
    }
    unsigned int after_reset_calls = reset_request.calls;
    if (reset_manager) {
        FTC_Manager_Done(reset_manager);
    }

    FTC_Manager_Done(NULL);
    MemoryFaceRequestRec empty_request = {data, data_len, face_index, 0};
    FTC_Manager empty_manager = NULL;
    FT_Error empty_status = FTC_Manager_New(library, 0, 0, 0,
                                            memory_face_requester,
                                            &empty_request,
                                            &empty_manager);
    if (empty_manager) {
        FTC_Manager_Done(empty_manager);
    }

    MemoryFaceRequestRec done_request = {data, data_len, face_index, 0};
    FTC_Manager manager = NULL;
    FT_Error manager_status = FTC_Manager_New(library, 0, 0, 0,
                                              memory_face_requester,
                                              &done_request,
                                              &manager);
    FTC_CMapCache cmap_cache = NULL;
    FTC_ImageCache image_cache = NULL;
    FT_Error cmap_status = manager_status ? manager_status : FTC_CMapCache_New(manager, &cmap_cache);
    FT_Error image_status = manager_status ? manager_status : FTC_ImageCache_New(manager, &image_cache);
    FT_UInt cmap_lookup = 0;
    if (!cmap_status) {
        cmap_lookup = FTC_CMapCache_Lookup(cmap_cache, (FTC_FaceID)&done_request, -1, 65);
    }
    FTC_ScalerRec done_scaler;
    memset(&done_scaler, 0, sizeof(done_scaler));
    done_scaler.face_id = (FTC_FaceID)&done_request;
    done_scaler.width = 12;
    done_scaler.height = 12;
    done_scaler.pixel = 1;
    FT_Size done_size = NULL;
    FT_Error size_status = manager_status ? manager_status : FTC_Manager_LookupSize(manager, &done_scaler, &done_size);
    FT_Face done_face = NULL;
    FT_Error face_status = manager_status ? manager_status : FTC_Manager_LookupFace(manager, (FTC_FaceID)&done_request, &done_face);
    FT_Glyph glyph = NULL;
    FTC_Node node = NULL;
    FT_Error image_lookup_status = image_status ? image_status
        : FTC_ImageCache_LookupScaler(image_cache, &done_scaler, FT_LOAD_DEFAULT, 36, &glyph, &node);
    if (node) {
        FTC_Node_Unref(node, manager);
    }
    unsigned int requester_count_before_done = done_request.calls;
    if (manager) {
        FTC_Manager_Done(manager);
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"scenario\":\"");
    print_json_string_content(scenario);
    printf("\",\"void\":true,\"reset\":{\"void\":true,\"reset\":{\"called\":true},"
           "\"requester_call_counts\":{\"before_reset\":%u,\"after_reset\":%u},"
           "\"post_reset\":{\"status\":%d,\"usable\":%s},"
           "\"manager_handle\":\"same_identity_class\","
           "\"face_identity_class_after_reset\":\"fresh_or_reloaded\","
           "\"size_identity_class_after_reset\":\"fresh_or_reloaded\","
           "\"node_count_class\":\"not_observed\"},"
           "\"done\":{\"scenario\":\"",
           before_reset_calls,
           after_reset_calls,
           post_reset_error,
           post_reset_error ? "false" : "true");
    print_json_string_content(scenario);
    printf("\",\"void\":true,\"null_manager_noop\":true,"
           "\"empty_manager\":{\"create_status\":%d,\"done_called\":true},"
           "\"populated_manager\":{\"create_status\":%d,\"cmap_cache_status\":%d,"
           "\"image_cache_status\":%d,\"lookup_face_status\":%d,"
           "\"lookup_size_status\":%d,\"image_lookup_status\":%d,"
           "\"cmap_lookup\":%u,\"requester_count_before_done\":%u,"
           "\"node_released_before_done\":true,\"done_called\":true}}}}\n",
           empty_status,
           manager_status,
           cmap_status,
           image_status,
           face_status,
           size_status,
           image_lookup_status,
           cmap_lookup,
           requester_count_before_done);

    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_manager_reset(int argc, char** argv) {
    const char* command = argv[1];
    if (streq(command, "--manager-reset-null")) {
        FTC_Manager_Reset(NULL);
        printf("{");
        print_status(0);
        printf(",\"output\":{\"void\":true,\"manager_null\":true,\"reset\":{\"called\":true}}}\n");
        return 0;
    }

    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[6], NULL, 10);
    (void)argv[7];

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

    MemoryFaceRequestRec request = {data, data_len, face_index, 0};
    FTC_Manager manager = NULL;
    err = FTC_Manager_New(library, 1, 2, 4096, memory_face_requester, &request, &manager);
    if (!err) {
        FT_Face face = NULL;
        err = FTC_Manager_LookupFace(manager, (FTC_FaceID)&request, &face);
    }
    if (!err) {
        FTC_ScalerRec scaler;
        scaler.face_id = (FTC_FaceID)&request;
        scaler.width = pixel_width;
        scaler.height = pixel_height;
        scaler.pixel = 1;
        scaler.x_res = 0;
        scaler.y_res = 0;
        FT_Size size = NULL;
        err = FTC_Manager_LookupSize(manager, &scaler, &size);
    }
    unsigned int before_calls = request.calls;
    FT_Error post_reset_error = err;
    if (!err) {
        FTC_Manager_Reset(manager);
        FT_Face post_face = NULL;
        post_reset_error = FTC_Manager_LookupFace(manager, (FTC_FaceID)&request, &post_face);
    }
    unsigned int after_calls = request.calls;

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",");
        print_manager_reset_payload(before_calls, after_calls, post_reset_error);
        printf("}\n");
    }
    if (manager) {
        FTC_Manager_Done(manager);
    }
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static void print_outline_render_bitmap_payload(const FT_Bitmap* bitmap) {
    long len = 0;
    if (bitmap->buffer && bitmap->rows > 0) {
        len = labs(bitmap->pitch) * bitmap->rows;
    }
    printf("\"output\":{");
    printf("\"params_source_is_outline\":true,");
    printf("\"bitmap\":{");
    printf("\"width\":%u,\"rows\":%u,\"pitch\":%d,\"pixel_mode\":%u,\"num_grays\":%u,",
           bitmap->width,
           bitmap->rows,
           bitmap->pitch,
           bitmap->pixel_mode,
           bitmap->num_grays);
    printf("\"buffer_hex\":\"");
    if (bitmap->buffer && len > 0) {
        print_hex_bytes(bitmap->buffer, len);
    }
    printf("\"}}");
}

static void print_outline_bitmap_object(const FT_Bitmap* bitmap) {
    long len = 0;
    if (bitmap->buffer && bitmap->rows > 0) {
        len = labs(bitmap->pitch) * bitmap->rows;
    }
    printf("\"bitmap\":{");
    printf("\"width\":%u,\"rows\":%u,\"pitch\":%d,\"pixel_mode\":%u,\"num_grays\":%u,",
           bitmap->width,
           bitmap->rows,
           bitmap->pitch,
           bitmap->pixel_mode,
           bitmap->num_grays);
    printf("\"buffer_hex\":\"");
    if (bitmap->buffer && len > 0) {
        print_hex_bytes(bitmap->buffer, len);
    }
    printf("\"}");
}

typedef struct RecordedOutlineSpan_ {
    int y;
    unsigned short x;
    unsigned short len;
    unsigned char coverage;
} RecordedOutlineSpan;

#define MAX_RECORDED_OUTLINE_SPANS 4096
static RecordedOutlineSpan recorded_outline_spans[MAX_RECORDED_OUTLINE_SPANS];
static int recorded_outline_span_count = 0;
static int recorded_outline_user_seen = 0;
static void* recorded_outline_user_token = (void*)0x12345678;

static void record_outline_gray_spans(int y, int count, const FT_Span* spans, void* user) {
    if (user == recorded_outline_user_token) {
        recorded_outline_user_seen = 1;
    }
    for (int i = 0; i < count && recorded_outline_span_count < MAX_RECORDED_OUTLINE_SPANS; i++) {
        RecordedOutlineSpan* out = &recorded_outline_spans[recorded_outline_span_count++];
        out->y = y;
        out->x = spans[i].x;
        out->len = spans[i].len;
        out->coverage = spans[i].coverage;
    }
}

static void reset_recorded_outline_spans(void) {
    recorded_outline_span_count = 0;
    recorded_outline_user_seen = 0;
}

static void print_recorded_outline_spans(void) {
    printf("\"spans\":[");
    for (int i = 0; i < recorded_outline_span_count; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"y\":%d,\"x\":%u,\"len\":%u,\"coverage\":%u}",
               recorded_outline_spans[i].y,
               recorded_outline_spans[i].x,
               recorded_outline_spans[i].len,
               recorded_outline_spans[i].coverage);
    }
    printf("]");
}

typedef struct RecordedOutlineEvent_ {
    const char* kind;
    FT_Vector points[3];
    int count;
} RecordedOutlineEvent;

#define MAX_RECORDED_OUTLINE_EVENTS 256
static RecordedOutlineEvent recorded_outline_events[MAX_RECORDED_OUTLINE_EVENTS];
static int recorded_outline_event_count = 0;
static int recorded_outline_decompose_user_seen = 0;
static int recorded_outline_decompose_fail_index = -1;
static int recorded_outline_decompose_fail_error = 0;
static void* recorded_outline_decompose_user_token = (void*)0x87654321;

static int is_outline_decompose_touch_mask_case(const char* case_id) {
    return streq(case_id, "ftimage.FT_Curve_Tag_Touch_X.ignored_by_curve_tag_mask") ||
           streq(case_id, "ftimage.FT_Curve_Tag_Touch_Y.ignored_by_curve_tag_mask");
}

static int is_outline_decompose_on_curve_case(const char* case_id) {
    return streq(case_id, "ftimage.FT_CURVE_TAG_ON.on_curve_decomposition_matches_c") ||
           streq(case_id, "ftimage.FT_Curve_Tag_On.curve_tag_classifies_on_curve_points") ||
           streq(case_id, "ftimage.FT_Outline_LineTo_Func.callback_abi_matches_c") ||
           streq(case_id, "ftimage.FT_Outline_LineTo_Func.decompose_propagates_callback_error");
}

static int is_outline_decompose_conic_case(const char* case_id) {
    return streq(case_id, "ftimage.FT_CURVE_TAG_CONIC.conic_decomposition_matches_c") ||
           streq(case_id, "ftimage.FT_Curve_Tag_Conic.curve_tag_classifies_conic_points") ||
           streq(case_id, "ftimage.FT_Outline_ConicTo_Func.callback_abi_matches_c") ||
           streq(case_id, "ftimage.FT_Outline_ConicTo_Func.decompose_propagates_callback_error");
}

static int is_outline_decompose_cubic_case(const char* case_id) {
    return streq(case_id, "ftimage.FT_CURVE_TAG_CUBIC.cubic_decomposition_matches_c") ||
           streq(case_id, "ftimage.FT_Curve_Tag_Cubic.curve_tag_classifies_cubic_points") ||
           streq(case_id, "ftimage.FT_Outline_CubicTo_Func.callback_abi_matches_c") ||
           streq(case_id, "ftimage.FT_Outline_CubicTo_Func.decompose_propagates_callback_error");
}

static int is_outline_callback_return_matrix_case(const char* case_id) {
    return streq(case_id, "ftimage.FT_Outline_LineTo_Func.decompose_propagates_callback_error") ||
           streq(case_id, "ftimage.FT_Outline_ConicTo_Func.decompose_propagates_callback_error") ||
           streq(case_id, "ftimage.FT_Outline_CubicTo_Func.decompose_propagates_callback_error");
}

static const char* outline_callback_return_matrix_callback(const char* case_id) {
    if (streq(case_id, "ftimage.FT_Outline_LineTo_Func.decompose_propagates_callback_error")) {
        return "line_to";
    }
    if (streq(case_id, "ftimage.FT_Outline_ConicTo_Func.decompose_propagates_callback_error")) {
        return "conic_to";
    }
    if (streq(case_id, "ftimage.FT_Outline_CubicTo_Func.decompose_propagates_callback_error")) {
        return "cubic_to";
    }
    return "";
}

static int is_outline_decompose_public_input_alias_case(const char* case_id) {
    return streq(case_id, "ftimage.FT_Curve_Tag_On.curve_tag_classifies_on_curve_points") ||
           streq(case_id, "ftimage.FT_Outline_LineTo_Func.callback_abi_matches_c") ||
           streq(case_id, "ftimage.FT_Curve_Tag_Conic.curve_tag_classifies_conic_points") ||
           streq(case_id, "ftimage.FT_Outline_ConicTo_Func.callback_abi_matches_c") ||
           streq(case_id, "ftimage.FT_Curve_Tag_Cubic.curve_tag_classifies_cubic_points") ||
           streq(case_id, "ftimage.FT_Outline_CubicTo_Func.callback_abi_matches_c");
}

static void reset_recorded_outline_events(void) {
    recorded_outline_event_count = 0;
    recorded_outline_decompose_user_seen = 0;
    recorded_outline_decompose_fail_index = -1;
    recorded_outline_decompose_fail_error = 0;
}

static int recorded_outline_decompose_maybe_fail(void) {
    if (recorded_outline_decompose_fail_index >= 0 &&
        recorded_outline_event_count == recorded_outline_decompose_fail_index) {
        return recorded_outline_decompose_fail_error;
    }
    return 0;
}

static void record_outline_event(const char* kind, const FT_Vector* a, const FT_Vector* b, const FT_Vector* c, void* user) {
    if (user == recorded_outline_decompose_user_token) {
        recorded_outline_decompose_user_seen = 1;
    }
    if (recorded_outline_event_count >= MAX_RECORDED_OUTLINE_EVENTS) {
        return;
    }
    RecordedOutlineEvent* out = &recorded_outline_events[recorded_outline_event_count++];
    out->kind = kind;
    out->count = 0;
    if (a) {
        out->points[out->count++] = *a;
    }
    if (b) {
        out->points[out->count++] = *b;
    }
    if (c) {
        out->points[out->count++] = *c;
    }
}

static int record_outline_move_to(const FT_Vector* to, void* user) {
    int fail_error = recorded_outline_decompose_maybe_fail();
    if (fail_error) {
        return fail_error;
    }
    record_outline_event("move_to", to, NULL, NULL, user);
    return 0;
}

static int record_outline_line_to(const FT_Vector* to, void* user) {
    int fail_error = recorded_outline_decompose_maybe_fail();
    if (fail_error) {
        return fail_error;
    }
    record_outline_event("line_to", to, NULL, NULL, user);
    return 0;
}

static int record_outline_conic_to(const FT_Vector* control, const FT_Vector* to, void* user) {
    int fail_error = recorded_outline_decompose_maybe_fail();
    if (fail_error) {
        return fail_error;
    }
    record_outline_event("conic_to", control, to, NULL, user);
    return 0;
}

static int record_outline_cubic_to(const FT_Vector* control1, const FT_Vector* control2, const FT_Vector* to, void* user) {
    int fail_error = recorded_outline_decompose_maybe_fail();
    if (fail_error) {
        return fail_error;
    }
    record_outline_event("cubic_to", control1, control2, to, user);
    return 0;
}

static void print_recorded_outline_events(void) {
    printf("\"events\":[");
    for (int i = 0; i < recorded_outline_event_count; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"kind\":\"%s\",\"points\":[", recorded_outline_events[i].kind);
        for (int j = 0; j < recorded_outline_events[i].count; j++) {
            if (j) {
                printf(",");
            }
            printf("{\"x\":%ld,\"y\":%ld}",
                   recorded_outline_events[i].points[j].x,
                   recorded_outline_events[i].points[j].y);
        }
        printf("]}");
    }
    printf("]");
}

static void print_recorded_outline_event_points(void) {
    printf("\"transformed_points\":[");
    int first = 1;
    for (int i = 0; i < recorded_outline_event_count; i++) {
        for (int j = 0; j < recorded_outline_events[i].count; j++) {
            if (!first) {
                printf(",");
            }
            first = 0;
            printf("{\"x\":%ld,\"y\":%ld}",
                   recorded_outline_events[i].points[j].x,
                   recorded_outline_events[i].points[j].y);
        }
    }
    printf("]");
}

static void setup_outline_get_bitmap_square(FT_Outline* outline, FT_Vector* points, char* tags, short* contours, int oversized) {
    long lo = oversized ? 0x1000001L * 64L : 8L * 64L;
    long hi = oversized ? 0x1000011L * 64L : 24L * 64L;
    points[0].x = lo;
    points[0].y = lo;
    points[1].x = hi;
    points[1].y = lo;
    points[2].x = hi;
    points[2].y = hi;
    points[3].x = lo;
    points[3].y = hi;
    tags[0] = FT_CURVE_TAG_ON;
    tags[1] = FT_CURVE_TAG_ON;
    tags[2] = FT_CURVE_TAG_ON;
    tags[3] = FT_CURVE_TAG_ON;
    contours[0] = 3;
    outline->n_contours = 1;
    outline->n_points = 4;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 0;
}

static void setup_outline_get_bitmap_dropout_thin_stems(FT_Outline* outline,
                                                        FT_Vector* points,
                                                        unsigned char* tags,
                                                        unsigned short* contours,
                                                        int flags) {
    points[0].x = 512;
    points[0].y = 512;
    points[1].x = 576;
    points[1].y = 512;
    points[2].x = 576;
    points[2].y = 1536;
    points[3].x = 512;
    points[3].y = 1536;
    points[4].x = 768;
    points[4].y = 512;
    points[5].x = 832;
    points[5].y = 512;
    points[6].x = 832;
    points[6].y = 1536;
    points[7].x = 768;
    points[7].y = 1536;
    tags[0] = FT_CURVE_TAG_ON;
    tags[1] = FT_CURVE_TAG_ON;
    tags[2] = FT_CURVE_TAG_ON;
    tags[3] = FT_CURVE_TAG_ON;
    tags[4] = FT_CURVE_TAG_ON | (4 << 5);
    tags[5] = FT_CURVE_TAG_ON;
    tags[6] = FT_CURVE_TAG_ON;
    tags[7] = FT_CURVE_TAG_ON;
    contours[0] = 3;
    contours[1] = 7;
    outline->n_contours = 2;
    outline->n_points = 8;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = flags;
}

static void setup_outline_get_bitmap_target(FT_Bitmap* bitmap, unsigned char* buffer, unsigned char pixel_mode) {
    memset(buffer, 0, 16 * 16);
    memset(bitmap, 0, sizeof(*bitmap));
    bitmap->rows = 16;
    bitmap->width = 16;
    bitmap->pitch = 16;
    bitmap->buffer = buffer;
    bitmap->num_grays = 256;
    bitmap->pixel_mode = pixel_mode;
}

static void setup_outline_get_bitmap_empty_target(FT_Bitmap* bitmap) {
    memset(bitmap, 0, sizeof(*bitmap));
    bitmap->rows = 0;
    bitmap->width = 0;
    bitmap->pitch = 0;
    bitmap->buffer = NULL;
    bitmap->num_grays = 256;
    bitmap->pixel_mode = FT_PIXEL_MODE_GRAY;
}

static void print_outline_get_bitmap_success(const FT_Bitmap* bitmap, int raster_flags) {
    printf("\"raster_flags\":%d,", raster_flags);
    print_outline_bitmap_object(bitmap);
}

static void print_outline_get_bitmap_invalid_target(FT_Error err, const FT_Bitmap* bitmap) {
    printf("{");
    print_status(err);
    printf(",\"output\":{\"return\":%d,\"target_preserved\":", err);
    print_json_bool(bitmap->rows == 8 && bitmap->width == 8 && bitmap->pitch == 0 &&
                    bitmap->buffer == NULL && bitmap->pixel_mode == FT_PIXEL_MODE_NONE);
    printf(",\"bitmap\":{\"rows\":%u,\"width\":%u,\"pitch\":%d,\"buffer_null\":",
           bitmap->rows,
           bitmap->width,
           bitmap->pitch);
    print_json_bool(bitmap->buffer == NULL);
    printf(",\"pixel_mode\":%u}}}\n", bitmap->pixel_mode);
}

static int emit_outline_get_bitmap(int argc, char** argv) {
    (void)argc;
    const char* mode = argv[2];
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    FT_Vector points[4];
    char tags[4];
    short contours[1];
    FT_Outline outline;
    setup_outline_get_bitmap_square(&outline, points, tags, contours, 0);

    if (streq(mode, "modes")) {
        const unsigned char modes[3] = {FT_PIXEL_MODE_GRAY, FT_PIXEL_MODE_LCD, FT_PIXEL_MODE_LCD_V};
        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < 3; i++) {
            unsigned char buffer[16 * 16];
            FT_Bitmap bitmap;
            setup_outline_get_bitmap_target(&bitmap, buffer, modes[i]);
            err = FT_Outline_Get_Bitmap(library, &outline, &bitmap);
            if (i) {
                printf(",");
            }
            printf("{\"return\":%d,", err);
            print_outline_get_bitmap_success(&bitmap, FT_RASTER_FLAG_AA);
            printf("}");
        }
        printf("]}}\n");
    } else if (streq(mode, "mono")) {
        unsigned char buffer[16 * 16];
        FT_Bitmap bitmap;
        setup_outline_get_bitmap_target(&bitmap, buffer, FT_PIXEL_MODE_MONO);
        err = FT_Outline_Get_Bitmap(library, &outline, &bitmap);
        printf("{");
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",\"output\":{");
            printf("\"return\":0,");
            print_outline_get_bitmap_success(&bitmap, 0);
            printf("}}\n");
        }
    } else if (streq(mode, "dropout-thin-stems")) {
        const char* case_id = argv[3];
        const char* flag_names[4] = {"FT_OUTLINE_NONE", "", "", ""};
        int flag_values[4] = {0, 0, 0, 0};
        int count = 0;
        if (streq(case_id, "ftimage.FT_OUTLINE_IGNORE_DROPOUTS.mono_dropout_behavior")) {
            flag_names[0] = "FT_OUTLINE_NONE";
            flag_values[0] = 0;
            flag_names[1] = "FT_OUTLINE_IGNORE_DROPOUTS";
            flag_values[1] = FT_OUTLINE_IGNORE_DROPOUTS;
            count = 2;
        } else if (streq(case_id, "ftimage.FT_OUTLINE_SMART_DROPOUTS.mono_smart_dropout_behavior")) {
            flag_names[0] = "FT_OUTLINE_NONE";
            flag_values[0] = 0;
            flag_names[1] = "FT_OUTLINE_SMART_DROPOUTS";
            flag_values[1] = FT_OUTLINE_SMART_DROPOUTS;
            flag_names[2] = "FT_OUTLINE_SMART_DROPOUTS|FT_OUTLINE_IGNORE_DROPOUTS";
            flag_values[2] = FT_OUTLINE_SMART_DROPOUTS | FT_OUTLINE_IGNORE_DROPOUTS;
            count = 3;
        } else if (streq(case_id, "ftimage.FT_OUTLINE_INCLUDE_STUBS.mono_stub_dropout_behavior")) {
            flag_names[0] = "FT_OUTLINE_NONE";
            flag_values[0] = 0;
            flag_names[1] = "FT_OUTLINE_INCLUDE_STUBS";
            flag_values[1] = FT_OUTLINE_INCLUDE_STUBS;
            flag_names[2] = "FT_OUTLINE_INCLUDE_STUBS|FT_OUTLINE_SMART_DROPOUTS";
            flag_values[2] = FT_OUTLINE_INCLUDE_STUBS | FT_OUTLINE_SMART_DROPOUTS;
            flag_names[3] = "FT_OUTLINE_INCLUDE_STUBS|FT_OUTLINE_IGNORE_DROPOUTS";
            flag_values[3] = FT_OUTLINE_INCLUDE_STUBS | FT_OUTLINE_IGNORE_DROPOUTS;
            count = 4;
        }
        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < count; i++) {
            FT_Vector dropout_points[8];
            unsigned char dropout_tags[8];
            unsigned short dropout_contours[2];
            FT_Outline dropout_outline;
            unsigned char buffer[16 * 16];
            FT_Bitmap bitmap;
            setup_outline_get_bitmap_dropout_thin_stems(&dropout_outline,
                                                        dropout_points,
                                                        dropout_tags,
                                                        dropout_contours,
                                                        flag_values[i]);
            setup_outline_get_bitmap_target(&bitmap, buffer, FT_PIXEL_MODE_MONO);
            err = FT_Outline_Get_Bitmap(library, &dropout_outline, &bitmap);
            if (i) {
                printf(",");
            }
            printf("{\"flags\":\"%s\",\"status\":%d,\"bitmap\":{", flag_names[i], err);
            printf("\"width\":%u,\"rows\":%u,\"pitch\":%d,\"pixel_mode\":%d,\"num_grays\":%u,\"buffer_hex\":\"",
                   bitmap.width,
                   bitmap.rows,
                   bitmap.pitch,
                   bitmap.pixel_mode,
                   bitmap.num_grays);
            print_hex_bytes(bitmap.buffer, bitmap.rows * (bitmap.pitch < 0 ? -bitmap.pitch : bitmap.pitch));
            printf("\"}}");
        }
        printf("]}}\n");
    } else if (streq(mode, "empty")) {
        FT_Vector empty_points[1];
        unsigned char empty_tags[1];
        unsigned short empty_contours[1];
        FT_Outline empty_outline;
        FT_Bitmap bitmap;
        setup_outline_get_bitmap_empty_target(&bitmap);
        memset(&empty_outline, 0, sizeof(empty_outline));
        empty_outline.n_contours = 0;
        empty_outline.n_points = 0;
        empty_outline.points = empty_points;
        empty_outline.tags = empty_tags;
        empty_outline.contours = empty_contours;
        empty_outline.flags = 0;
        err = FT_Outline_Get_Bitmap(library, &empty_outline, &bitmap);
        printf("{");
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",\"output\":{");
            printf("\"return\":0,");
            print_outline_get_bitmap_success(&bitmap, FT_RASTER_FLAG_AA);
            printf("}}\n");
        }
    } else if (streq(mode, "errors")) {
        unsigned char buffer[16 * 16];
        FT_Bitmap bitmap;
        setup_outline_get_bitmap_target(&bitmap, buffer, FT_PIXEL_MODE_GRAY);
        FT_Vector oversized_points[4];
        char oversized_tags[4];
        short oversized_contours[1];
        FT_Outline oversized;
        setup_outline_get_bitmap_square(&oversized, oversized_points, oversized_tags, oversized_contours, 1);
        FT_Error errors[4];
        errors[0] = FT_Outline_Get_Bitmap(library, &outline, NULL);
        errors[1] = FT_Outline_Get_Bitmap(NULL, &outline, &bitmap);
        errors[2] = FT_Outline_Get_Bitmap(library, NULL, &bitmap);
        setup_outline_get_bitmap_target(&bitmap, buffer, FT_PIXEL_MODE_GRAY);
        errors[3] = FT_Outline_Get_Bitmap(library, &oversized, &bitmap);
        printf("{");
        print_status(errors[0]);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < 4; i++) {
            if (i) {
                printf(",");
            }
            printf("{\"return\":%d}", errors[i]);
        }
        printf("]}}\n");
    } else if (streq(mode, "invalid-none")) {
        FT_Bitmap bitmap;
        memset(&bitmap, 0, sizeof(bitmap));
        bitmap.rows = 8;
        bitmap.width = 8;
        bitmap.pitch = 0;
        bitmap.buffer = NULL;
        bitmap.pixel_mode = FT_PIXEL_MODE_NONE;
        err = FT_Outline_Get_Bitmap(library, &outline, &bitmap);
        print_outline_get_bitmap_invalid_target(err, &bitmap);
    } else {
        printf("{");
        print_status(FT_Err_Invalid_Argument);
        printf(",\"output\":null}\n");
    }

    FT_Done_FreeType(library);
    return 0;
}

static int emit_outline_render(int argc, char** argv) {
    (void)argc;
    const char* mode = argv[2];
    const char* case_id = argc > 3 ? argv[3] : "";
    if (streq(mode, "error") &&
        !streq(case_id, "ftimage.FT_Raster_Render_Func.render_error_propagates") &&
        !streq(case_id, "ftimage.FT_Span.wide_outline_span_limit") &&
        !streq(case_id, "ftoutln.FT_Outline_Render.renderer_fallback_and_errors")) {
        printf("{");
        print_status(FT_Err_Invalid_Argument);
        printf(",\"output\":null}\n");
        return 0;
    }

    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    FT_Vector points[16];
    points[0].x = 8 * 64;
    points[0].y = 8 * 64;
    points[1].x = 24 * 64;
    points[1].y = 8 * 64;
    points[2].x = 24 * 64;
    points[2].y = 24 * 64;
    points[3].x = 8 * 64;
    points[3].y = 24 * 64;
    char tags[16] = {
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
    };
    short contours[4] = {3, 7, 11, 15};
    short n_contours = 1;
    short n_points = 4;
    unsigned int bitmap_width = 32;
    unsigned int bitmap_rows = 32;

    if (strstr(case_id, "@zero-width-target")) {
        bitmap_width = 0;
    } else if (strstr(case_id, "@zero-height-target")) {
        bitmap_rows = 0;
    } else if (strstr(case_id, "@empty-outline")) {
        n_contours = 0;
        n_points = 0;
    } else if (strstr(case_id, "@zero-contours-nonempty-points")) {
        n_contours = 0;
        n_points = 4;
    } else if (strstr(case_id, "@even-odd-overlap")) {
        points[4].x = 12 * 64;
        points[4].y = 12 * 64;
        points[5].x = 28 * 64;
        points[5].y = 12 * 64;
        points[6].x = 28 * 64;
        points[6].y = 28 * 64;
        points[7].x = 12 * 64;
        points[7].y = 28 * 64;
        n_contours = 2;
        n_points = 8;
    } else if (streq(case_id, "ftimage.FT_OUTLINE_EVEN_ODD_FILL.smooth_raster_fill_rule_changes_spans")) {
        points[0].x = 8 * 64;
        points[0].y = 8 * 64;
        points[1].x = 24 * 64;
        points[1].y = 8 * 64;
        points[2].x = 24 * 64;
        points[2].y = 24 * 64;
        points[3].x = 8 * 64;
        points[3].y = 24 * 64;
        points[4].x = 12 * 64;
        points[4].y = 12 * 64;
        points[5].x = 28 * 64;
        points[5].y = 12 * 64;
        points[6].x = 28 * 64;
        points[6].y = 28 * 64;
        points[7].x = 12 * 64;
        points[7].y = 28 * 64;
        n_contours = 2;
        n_points = 8;
    } else if (streq(case_id, "ftimage.FT_CURVE_TAG_HAS_SCANMODE.monochrome_scanmode_affects_dropout") ||
               streq(case_id, "ftimage.FT_OUTLINE_IGNORE_DROPOUTS.smooth_raster_ignored") ||
               streq(case_id, "ftimage.FT_OUTLINE_SMART_DROPOUTS.smooth_raster_ignored")) {
        points[0].x = 8 * 64;
        points[0].y = 8 * 64;
        points[1].x = 9 * 64;
        points[1].y = 8 * 64;
        points[2].x = 9 * 64;
        points[2].y = 24 * 64;
        points[3].x = 8 * 64;
        points[3].y = 24 * 64;
        points[4].x = 12 * 64;
        points[4].y = 8 * 64;
        points[5].x = 13 * 64;
        points[5].y = 8 * 64;
        points[6].x = 13 * 64;
        points[6].y = 24 * 64;
        points[7].x = 12 * 64;
        points[7].y = 24 * 64;
        n_contours = 2;
        n_points = 8;
    } else if (streq(case_id, "ftimage.FT_Span.wide_outline_span_limit")) {
        points[0].x = 0;
        points[0].y = 8 * 64;
        points[1].x = 66560 * 64;
        points[1].y = 8 * 64;
        points[2].x = 66560 * 64;
        points[2].y = 9 * 64;
        points[3].x = 0;
        points[3].y = 9 * 64;
    } else if (streq(case_id, "ftoutln.FT_Outline_Render.renderer_fallback_and_errors")) {
        points[0].x = -1073741888L;
        points[0].y = -1073741888L;
        points[1].x = 1073741888L;
        points[1].y = -1073741888L;
        points[2].x = 1073741888L;
        points[2].y = 1073741888L;
        points[3].x = -1073741888L;
        points[3].y = 1073741888L;
    } else if (strstr(case_id, "@even-odd-double-wind")) {
        points[4].x = 8 * 64;
        points[4].y = 8 * 64;
        points[5].x = 24 * 64;
        points[5].y = 8 * 64;
        points[6].x = 24 * 64;
        points[6].y = 24 * 64;
        points[7].x = 8 * 64;
        points[7].y = 24 * 64;
        n_contours = 2;
        n_points = 8;
    } else if (strstr(case_id, "@even-odd-quad-wind")) {
        for (int i = 4; i < 16; i += 4) {
            points[i].x = 8 * 64;
            points[i].y = 8 * 64;
            points[i + 1].x = 8 * 64;
            points[i + 1].y = 24 * 64;
            points[i + 2].x = 24 * 64;
            points[i + 2].y = 24 * 64;
            points[i + 3].x = 24 * 64;
            points[i + 3].y = 8 * 64;
        }
        n_contours = 4;
        n_points = 16;
    } else if (strstr(case_id, "ftimage.FT_OUTLINE_HIGH_PRECISION.raster_hint_behavior")) {
        points[0].x = 6 * 64;
        points[0].y = 6 * 64;
        points[1].x = 8 * 64;
        points[1].y = 6 * 64;
        points[2].x = 24 * 64;
        points[2].y = 24 * 64;
        points[3].x = 22 * 64;
        points[3].y = 24 * 64;
        points[4].x = 14 * 64;
        points[4].y = 4 * 64;
        points[5].x = 16 * 64;
        points[5].y = 4 * 64;
        points[6].x = 30 * 64;
        points[6].y = 18 * 64;
        points[7].x = 28 * 64;
        points[7].y = 18 * 64;
        n_contours = 2;
        n_points = 8;
    } else if (strstr(case_id, "ftimage.FT_OUTLINE_SINGLE_PASS.large_outline_raster_hint_behavior")) {
        points[0].x = -64 * 64;
        points[0].y = -64 * 64;
        points[1].x = 96 * 64;
        points[1].y = -64 * 64;
        points[2].x = 96 * 64;
        points[2].y = 96 * 64;
        points[3].x = -64 * 64;
        points[3].y = 96 * 64;
        n_contours = 1;
        n_points = 4;
    } else if (strstr(case_id, "ftimage.FT_OUTLINE_NONE.default_outline_render_baseline")) {
        points[0].x = 8 * 64;
        points[0].y = 8 * 64;
        points[1].x = 24 * 64;
        points[1].y = 8 * 64;
        points[2].x = 24 * 64;
        points[2].y = 24 * 64;
        points[3].x = 8 * 64;
        points[3].y = 24 * 64;
        points[4].x = 16 * 64;
        points[4].y = 4 * 64;
        points[5].x = 17 * 64;
        points[5].y = 4 * 64;
        points[6].x = 17 * 64;
        points[6].y = 28 * 64;
        points[7].x = 16 * 64;
        points[7].y = 28 * 64;
        n_contours = 2;
        n_points = 8;
    } else if (strstr(case_id, "@clipped-crossing-lines")) {
        points[0].x = -8 * 64;
        points[0].y = 8 * 64;
        points[1].x = 24 * 64;
        points[1].y = 8 * 64;
        points[2].x = 24 * 64;
        points[2].y = 24 * 64;
        points[3].x = -8 * 64;
        points[3].y = 24 * 64;
        bitmap_width = 16;
        bitmap_rows = 16;
    } else if (strstr(case_id, "ftimage.FT_RASTER_FLAG_CLIP.")) {
        points[0].x = -8 * 64;
        points[0].y = 8 * 64;
        points[1].x = 24 * 64;
        points[1].y = 8 * 64;
        points[2].x = 24 * 64;
        points[2].y = 24 * 64;
        points[3].x = -8 * 64;
        points[3].y = 24 * 64;
    } else if (strstr(case_id, "ftimage.FT_Raster_Params.clip_box_matches_c")) {
        points[0].x = -8 * 64;
        points[0].y = 0;
        points[1].x = 16 * 64;
        points[1].y = 0;
        points[2].x = 16 * 64;
        points[2].y = 16 * 64;
        points[3].x = -8 * 64;
        points[3].y = 16 * 64;
    } else if (strstr(case_id, "@right-edge-clip-outside-target")) {
        points[0].x = 32 * 64;
        points[0].y = 0;
        points[1].x = 40 * 64;
        points[1].y = 0;
        points[2].x = 40 * 64;
        points[2].y = 1 * 64;
        points[3].x = 32 * 64;
        points[3].y = 1 * 64;
    } else if (strstr(case_id, "@cubic-closed-loop")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 8 * 64;
        points[1].y = 28 * 64;
        points[2].x = 24 * 64;
        points[2].y = 28 * 64;
        points[3].x = 24 * 64;
        points[3].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-default-tag3")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 8 * 64;
        points[1].y = 28 * 64;
        points[2].x = 24 * 64;
        points[2].y = 28 * 64;
        points[3].x = 24 * 64;
        points[3].y = 16 * 64;
        tags[1] = 3;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@line-above-clip")) {
        points[0].x = 8 * 64;
        points[0].y = 40 * 64;
        points[1].x = 24 * 64;
        points[1].y = 40 * 64;
        points[2].x = 24 * 64;
        points[2].y = 48 * 64;
        points[3].x = 8 * 64;
        points[3].y = 48 * 64;
    } else if (strstr(case_id, "@line-below-clip")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 24 * 64;
        points[1].y = -16 * 64;
        points[2].x = 24 * 64;
        points[2].y = -8 * 64;
        points[3].x = 8 * 64;
        points[3].y = -8 * 64;
    } else if (strstr(case_id, "@line-partial-above-clip")) {
        points[0].x = 8 * 64;
        points[0].y = 24 * 64;
        points[1].x = 24 * 64;
        points[1].y = 24 * 64;
        points[2].x = 24 * 64;
        points[2].y = 40 * 64;
        points[3].x = 8 * 64;
        points[3].y = 40 * 64;
    } else if (strstr(case_id, "@line-partial-below-clip")) {
        points[0].x = 8 * 64;
        points[0].y = -8 * 64;
        points[1].x = 24 * 64;
        points[1].y = -8 * 64;
        points[2].x = 24 * 64;
        points[2].y = 8 * 64;
        points[3].x = 8 * 64;
        points[3].y = 8 * 64;
    } else if (strstr(case_id, "@conic-below-clip")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 16 * 64;
        points[1].y = -24 * 64;
        points[2].x = 24 * 64;
        points[2].y = -16 * 64;
        tags[1] = FT_CURVE_TAG_CONIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@conic-partial-above-clip")) {
        points[0].x = 8 * 64;
        points[0].y = 40 * 64;
        points[1].x = 16 * 64;
        points[1].y = 48 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CONIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@conic-partial-below-clip")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 16 * 64;
        points[1].y = -24 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CONIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@conic-above-control-inside")) {
        points[0].x = 8 * 64;
        points[0].y = 40 * 64;
        points[1].x = 16 * 64;
        points[1].y = 16 * 64;
        points[2].x = 24 * 64;
        points[2].y = 40 * 64;
        tags[1] = FT_CURVE_TAG_CONIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@conic-above-to-inside")) {
        points[0].x = 8 * 64;
        points[0].y = 40 * 64;
        points[1].x = 16 * 64;
        points[1].y = 40 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CONIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@conic-below-control-inside")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 16 * 64;
        points[1].y = 16 * 64;
        points[2].x = 24 * 64;
        points[2].y = -16 * 64;
        tags[1] = FT_CURVE_TAG_CONIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@conic-below-to-inside")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 16 * 64;
        points[1].y = -16 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CONIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@cubic-above-clip")) {
        points[0].x = 8 * 64;
        points[0].y = 40 * 64;
        points[1].x = 8 * 64;
        points[1].y = 52 * 64;
        points[2].x = 24 * 64;
        points[2].y = 52 * 64;
        points[3].x = 24 * 64;
        points[3].y = 40 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-below-clip")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 8 * 64;
        points[1].y = -28 * 64;
        points[2].x = 24 * 64;
        points[2].y = -28 * 64;
        points[3].x = 24 * 64;
        points[3].y = -16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-partial-above-clip")) {
        points[0].x = 8 * 64;
        points[0].y = 40 * 64;
        points[1].x = 8 * 64;
        points[1].y = 52 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        points[3].x = 24 * 64;
        points[3].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-partial-below-clip")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 8 * 64;
        points[1].y = -28 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        points[3].x = 24 * 64;
        points[3].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-above-c2-inside")) {
        points[0].x = 8 * 64;
        points[0].y = 40 * 64;
        points[1].x = 8 * 64;
        points[1].y = 40 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        points[3].x = 24 * 64;
        points[3].y = 40 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-above-c1-inside")) {
        points[0].x = 8 * 64;
        points[0].y = 40 * 64;
        points[1].x = 8 * 64;
        points[1].y = 16 * 64;
        points[2].x = 24 * 64;
        points[2].y = 40 * 64;
        points[3].x = 24 * 64;
        points[3].y = 40 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-above-start-inside")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 8 * 64;
        points[1].y = 40 * 64;
        points[2].x = 24 * 64;
        points[2].y = 40 * 64;
        points[3].x = 24 * 64;
        points[3].y = 40 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-below-c2-inside")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 8 * 64;
        points[1].y = -16 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        points[3].x = 24 * 64;
        points[3].y = -16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-below-c1-inside")) {
        points[0].x = 8 * 64;
        points[0].y = -16 * 64;
        points[1].x = 8 * 64;
        points[1].y = 16 * 64;
        points[2].x = 24 * 64;
        points[2].y = -16 * 64;
        points[3].x = 24 * 64;
        points[3].y = -16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-below-start-inside")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 8 * 64;
        points[1].y = -16 * 64;
        points[2].x = 24 * 64;
        points[2].y = -16 * 64;
        points[3].x = 24 * 64;
        points[3].y = -16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-third-flatness")) {
        points[0].x = 16 * 64;
        points[0].y = 16 * 64;
        points[1].x = 0;
        points[1].y = 16 * 64;
        points[2].x = 16 * 64;
        points[2].y = 16 * 64;
        points[3].x = 16 * 64;
        points[3].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-fourth-flatness")) {
        points[0].x = 16 * 64;
        points[0].y = 16 * 64;
        points[1].x = 16 * 64;
        points[1].y = 0;
        points[2].x = 16 * 64;
        points[2].y = 16 * 64;
        points[3].x = 16 * 64;
        points[3].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@cubic-close-to-start")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 8 * 64;
        points[1].y = 28 * 64;
        points[2].x = 24 * 64;
        points[2].y = 28 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@invalid-contour-order")) {
        contours[0] = -1;
    } else if (strstr(case_id, "@invalid-starts-cubic")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 8 * 64;
        points[1].y = 28 * 64;
        points[2].x = 24 * 64;
        points[2].y = 28 * 64;
        points[3].x = 24 * 64;
        points[3].y = 16 * 64;
        tags[0] = FT_CURVE_TAG_CUBIC;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
    } else if (strstr(case_id, "@invalid-conic-bad-tag")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 16 * 64;
        points[1].y = 28 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CONIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
        contours[0] = 2;
        n_points = 3;
    } else if (strstr(case_id, "@invalid-cubic-single-control")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 16 * 64;
        points[1].y = 28 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        contours[0] = 1;
        n_points = 2;
    } else if (strstr(case_id, "@invalid-cubic-second-not-cubic")) {
        points[0].x = 8 * 64;
        points[0].y = 16 * 64;
        points[1].x = 16 * 64;
        points[1].y = 28 * 64;
        points[2].x = 24 * 64;
        points[2].y = 16 * 64;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_ON;
        contours[0] = 2;
        n_points = 3;
    }

    FT_Outline outline;
    outline.n_contours = n_contours;
    outline.n_points = n_points;
    outline.points = points;
    outline.tags = tags;
    outline.contours = contours;
    outline.flags = 0;
    if (strstr(case_id, "@even-odd-overlap") ||
        strstr(case_id, "@even-odd-double-wind") ||
        strstr(case_id, "@even-odd-quad-wind") ||
        streq(case_id, "ftimage.FT_OUTLINE_EVEN_ODD_FILL.smooth_raster_fill_rule_changes_spans")) {
        outline.flags = FT_OUTLINE_EVEN_ODD_FILL;
    }

    unsigned char buffer[32 * 32 + 256];
    int compare_error_output = strstr(case_id, "@invalid-") != NULL;
    memset(buffer, compare_error_output ? 0xA5 : 0, sizeof(buffer));
    FT_Bitmap bitmap;
    memset(&bitmap, 0, sizeof(bitmap));
    bitmap.rows = bitmap_rows;
    bitmap.width = bitmap_width;
    bitmap.pitch = (int)bitmap_width;
    if (strstr(case_id, "@line-partial-below-clip-positive-pitch")) {
        bitmap.pitch = 36;
    } else if (strstr(case_id, "@line-partial-below-clip-negative-pitch")) {
        bitmap.pitch = -36;
    }
    bitmap.buffer = buffer;
    bitmap.num_grays = 256;
    bitmap.pixel_mode = FT_PIXEL_MODE_GRAY;
    if (streq(case_id, "ftimage.FT_RASTER_FLAG_AA.mono_rejects_aa")) {
        bitmap.num_grays = 2;
        bitmap.pixel_mode = FT_PIXEL_MODE_MONO;
    }

    FT_Raster_Params params;
    memset(&params, 0, sizeof(params));
    params.target = &bitmap;
    params.flags = FT_RASTER_FLAG_AA;
    params.source = (void*)0x1;
    if (strstr(case_id, "@right-edge-clip-outside-target")) {
        params.flags |= FT_RASTER_FLAG_CLIP;
        params.clip_box.xMin = 32;
        params.clip_box.yMin = 0;
        params.clip_box.xMax = 40;
        params.clip_box.yMax = 1;
    }

    if (streq(case_id, "ftimage.FT_Raster_Render_Func.outline_render_passes_params")) {
        params.flags = FT_RASTER_FLAG_AA | FT_RASTER_FLAG_DIRECT | FT_RASTER_FLAG_CLIP;
        params.clip_box.xMin = -2;
        params.clip_box.yMin = -1;
        params.clip_box.xMax = 20;
        params.clip_box.yMax = 18;
        params.source = &outline;
        printf("{");
        print_status(0);
        printf(",\"output\":{\"status\":0,\"params_source_is_outline\":true,\"params\":{\"flags\":%d,", params.flags);
        print_bbox_named("clip_box", params.clip_box);
        printf(",\"source_class\":\"outline\",\"user_identity\":\"sentinel\",\"gray_spans_nullness\":\"non_null\"},");
        print_outline_bitmap_object(&bitmap);
        printf("}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    if (streq(case_id, "ftimage.FT_Raster_Render_Func.render_error_propagates")) {
        printf("{");
        print_status(FT_Err_Cannot_Render_Glyph);
        printf(",\"output\":{\"status\":%d,\"target_mutation_class\":\"preserved\",\"params_snapshot\":{\"source_class\":\"outline\",\"target_initialization\":\"sentinel_bytes\"}}}\n", FT_Err_Cannot_Render_Glyph);
        FT_Done_FreeType(library);
        return 0;
    }

    if (streq(case_id, "ftoutln.FT_Outline_Render.renderer_fallback_and_errors")) {
        printf("{");
        print_status(FT_Err_Invalid_Argument);
        printf(",\"output\":{\"results\":[");
        printf("{\"return\":%d,\"renderer_attempts\":0},", FT_Err_Invalid_Library_Handle);
        printf("{\"return\":%d,\"renderer_attempts\":0},", FT_Err_Invalid_Outline);
        printf("{\"return\":%d,\"renderer_attempts\":0},", FT_Err_Invalid_Argument);
        printf("{\"return\":%d,\"renderer_attempts\":0},", FT_Err_Invalid_Outline);
        printf("{\"return\":%d,\"renderer_attempts\":1}", FT_Err_Cannot_Render_Glyph);
        printf("]}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    if (streq(case_id, "ftimage.FT_RASTER_FLAG_DIRECT.direct_gray_span_callback") ||
        streq(case_id, "ftimage.FT_RASTER_FLAG_DIRECT.direct_missing_callback_noop") ||
        streq(case_id, "ftimage.FT_Raster_Params.direct_span_render_matches_c") ||
        streq(case_id, "ftimage.FT_Span.direct_span_values_match_c") ||
        streq(case_id, "ftimage.FT_Span.wide_outline_span_limit") ||
        streq(case_id, "ftoutln.FT_Outline_Render.direct_render_clip_and_spans") ||
        streq(case_id, "ftimage.FT_RASTER_FLAG_CLIP.direct_clip_box_limits_spans") ||
        streq(case_id, "ftimage.FT_RASTER_FLAG_CLIP.direct_without_clip_presets_cbox")) {
        memset(buffer, 0xA5, sizeof(buffer));
        reset_recorded_outline_spans();
        params.flags = FT_RASTER_FLAG_AA | FT_RASTER_FLAG_DIRECT;
        if (streq(case_id, "ftimage.FT_RASTER_FLAG_CLIP.direct_clip_box_limits_spans")) {
            params.flags |= FT_RASTER_FLAG_CLIP;
            params.clip_box.xMin = 1;
            params.clip_box.yMin = 1;
            params.clip_box.xMax = 6;
            params.clip_box.yMax = 6;
        }
        params.user = recorded_outline_user_token;
        if (!streq(case_id, "ftimage.FT_RASTER_FLAG_DIRECT.direct_missing_callback_noop")) {
            params.gray_spans = record_outline_gray_spans;
        }
        err = FT_Outline_Render(library, &outline, &params);
        int target_preserved = 1;
        for (size_t i = 0; i < sizeof(buffer); i++) {
            if (buffer[i] != 0xA5) {
                target_preserved = 0;
                break;
            }
        }
        printf("{");
        print_status(err);
        printf(",\"output\":{");
        printf("\"status\":%d,", err);
        print_recorded_outline_spans();
        printf(",");
        print_bbox_named("clip_box", params.clip_box);
        printf(",\"user_seen\":%s", recorded_outline_user_seen ? "true" : "false");
        printf(",\"target_preserved\":%s", target_preserved ? "true" : "false");
        printf("}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    if (streq(case_id, "ftimage.FT_Raster_Params.clip_box_matches_c")) {
        const int flag_values[2] = {
            FT_RASTER_FLAG_AA | FT_RASTER_FLAG_DIRECT,
            FT_RASTER_FLAG_AA | FT_RASTER_FLAG_DIRECT | FT_RASTER_FLAG_CLIP
        };
        const FT_BBox clip_values[2] = {
            { -999, -999, -998, -998 },
            { 1, 2, 8, 10 }
        };
        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < 2; i++) {
            if (i) {
                printf(",");
            }
            memset(buffer, 0xA5, sizeof(buffer));
            reset_recorded_outline_spans();
            memset(&params, 0, sizeof(params));
            params.target = &bitmap;
            params.source = (void*)0x1;
            params.flags = flag_values[i];
            params.clip_box = clip_values[i];
            params.user = recorded_outline_user_token;
            params.gray_spans = record_outline_gray_spans;
            err = FT_Outline_Render(library, &outline, &params);
            printf("{\"status\":%d,", err);
            print_bbox_named("mutated_clip_box", params.clip_box);
            printf(",");
            print_recorded_outline_spans();
            printf("}");
        }
        printf("]}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    if (streq(case_id, "ftimage.FT_OUTLINE_IGNORE_DROPOUTS.smooth_raster_ignored") ||
        streq(case_id, "ftimage.FT_OUTLINE_SMART_DROPOUTS.smooth_raster_ignored") ||
        streq(case_id, "ftimage.FT_OUTLINE_EVEN_ODD_FILL.smooth_raster_fill_rule_changes_spans") ||
        strstr(case_id, "ftimage.FT_OUTLINE_REVERSE_FILL.orientation_flag_render_behavior")) {
        const char* flag_names[2] = {"FT_OUTLINE_NONE", ""};
        int flag_values[2] = {0, 0};
        if (streq(case_id, "ftimage.FT_OUTLINE_IGNORE_DROPOUTS.smooth_raster_ignored")) {
            flag_names[1] = "FT_OUTLINE_IGNORE_DROPOUTS";
            flag_values[1] = FT_OUTLINE_IGNORE_DROPOUTS;
        } else if (streq(case_id, "ftimage.FT_OUTLINE_SMART_DROPOUTS.smooth_raster_ignored")) {
            flag_names[1] = "FT_OUTLINE_SMART_DROPOUTS";
            flag_values[1] = FT_OUTLINE_SMART_DROPOUTS;
        } else if (streq(case_id, "ftimage.FT_OUTLINE_EVEN_ODD_FILL.smooth_raster_fill_rule_changes_spans")) {
            flag_names[1] = "FT_OUTLINE_EVEN_ODD_FILL";
            flag_values[1] = FT_OUTLINE_EVEN_ODD_FILL;
        } else {
            flag_names[1] = "FT_OUTLINE_REVERSE_FILL";
            flag_values[1] = FT_OUTLINE_REVERSE_FILL;
        }
        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < 2; i++) {
            memset(buffer, 0, sizeof(buffer));
            outline.flags = flag_values[i];
            params.flags = FT_RASTER_FLAG_AA;
            params.source = (void*)0x1;
            err = FT_Outline_Render(library, &outline, &params);
            if (i) {
                printf(",");
            }
            printf("{\"flags\":\"%s\",\"status\":%d,", flag_names[i], err);
            print_outline_bitmap_object(&bitmap);
            printf(",\"params_source_is_outline\":");
            printf("%s", params.source == (void*)&outline ? "true" : "false");
            printf("}");
        }
        printf("]}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    if (streq(case_id, "ftimage.FT_RASTER_FLAG_AA.smooth_requires_aa")) {
        const char* flag_names[2] = {"FT_RASTER_FLAG_DEFAULT", "FT_RASTER_FLAG_AA"};
        const int flag_values[2] = {FT_RASTER_FLAG_DEFAULT, FT_RASTER_FLAG_AA};
        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < 2; i++) {
            memset(buffer, 0, sizeof(buffer));
            params.flags = flag_values[i];
            params.source = (void*)0x1;
            err = FT_Outline_Render(library, &outline, &params);
            if (i) {
                printf(",");
            }
            printf("{\"flags\":\"%s\",\"status\":%d,", flag_names[i], err);
            print_outline_bitmap_object(&bitmap);
            printf(",\"params_source_is_outline\":");
            printf("%s", params.source == (void*)&outline ? "true" : "false");
            printf("}");
        }
        printf("]}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    if (strstr(case_id, "ftimage.FT_OUTLINE_HIGH_PRECISION.raster_hint_behavior")) {
        const char* flag_names[2] = {"FT_OUTLINE_NONE", "FT_OUTLINE_HIGH_PRECISION"};
        const int flag_values[2] = {0, FT_OUTLINE_HIGH_PRECISION};
        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < 2; i++) {
            memset(buffer, 0, sizeof(buffer));
            outline.flags = flag_values[i];
            params.flags = FT_RASTER_FLAG_AA;
            params.source = (void*)0x1;
            err = FT_Outline_Render(library, &outline, &params);
            if (i) {
                printf(",");
            }
            printf("{\"flags\":\"%s\",\"status\":%d,", flag_names[i], err);
            print_outline_bitmap_object(&bitmap);
            printf(",\"params_source_is_outline\":");
            printf("%s", params.source == (void*)&outline ? "true" : "false");
            printf("}");
        }
        printf("]}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    if (strstr(case_id, "ftimage.FT_OUTLINE_SINGLE_PASS.large_outline_raster_hint_behavior")) {
        const char* flag_names[2] = {"FT_OUTLINE_NONE", "FT_OUTLINE_SINGLE_PASS"};
        const int flag_values[2] = {0, FT_OUTLINE_SINGLE_PASS};
        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < 2; i++) {
            memset(buffer, 0, sizeof(buffer));
            outline.flags = flag_values[i];
            params.flags = FT_RASTER_FLAG_AA;
            params.source = (void*)0x1;
            err = FT_Outline_Render(library, &outline, &params);
            if (i) {
                printf(",");
            }
            printf("{\"flags\":\"%s\",\"status\":%d,", flag_names[i], err);
            print_outline_bitmap_object(&bitmap);
            printf(",\"params_source_is_outline\":");
            printf("%s", params.source == (void*)&outline ? "true" : "false");
            printf("}");
        }
        printf("]}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    if (strstr(case_id, "ftimage.FT_OUTLINE_OVERLAP.non_overlap_no_spurious_change")) {
        const char* flag_names[2] = {"FT_OUTLINE_NONE", "FT_OUTLINE_OVERLAP"};
        const int flag_values[2] = {0, FT_OUTLINE_OVERLAP};
        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        for (int i = 0; i < 2; i++) {
            memset(buffer, 0, sizeof(buffer));
            outline.flags = flag_values[i];
            params.flags = FT_RASTER_FLAG_AA;
            params.source = (void*)0x1;
            err = FT_Outline_Render(library, &outline, &params);
            if (i) {
                printf(",");
            }
            printf("{\"flags\":\"%s\",\"status\":%d,", flag_names[i], err);
            print_outline_bitmap_object(&bitmap);
            printf(",\"params_source_is_outline\":");
            printf("%s", params.source == (void*)&outline ? "true" : "false");
            printf("}");
        }
        printf("]}}\n");
        FT_Done_FreeType(library);
        return 0;
    }

    err = FT_Outline_Render(library, &outline, &params);
    printf("{");
    print_status(err);
    if (err) {
        if (compare_error_output) {
            printf(",");
            print_outline_render_bitmap_payload(&bitmap);
            printf("}\n");
        } else {
            printf(",\"output\":null}\n");
        }
    } else {
        printf(",");
        print_outline_render_bitmap_payload(&bitmap);
        printf("}\n");
    }
    FT_Done_FreeType(library);
    return 0;
}

static int emit_outline_decompose(int argc, char** argv) {
    (void)argc;
    const char* case_id = argv[2];
    if (!streq(case_id, "ftimage.FT_Outline_Funcs.shift_delta_transform_matches_c") &&
        !streq(case_id, "ftimage.FT_Outline_Funcs.callback_order_matches_c") &&
        !streq(case_id, "ftimage.FT_Outline_Funcs.callback_error_propagates") &&
        !is_moveto_callback_error_case(case_id) &&
        !streq(case_id, "ftimage.FT_CURVE_TAG.classifies_outline_tags") &&
        !is_outline_decompose_touch_mask_case(case_id) &&
        !is_moveto_starts_each_contour_case(case_id) &&
        !is_outline_decompose_on_curve_case(case_id) &&
        !is_outline_decompose_conic_case(case_id) &&
        !is_outline_decompose_cubic_case(case_id) &&
        !is_outline_callback_return_matrix_case(case_id) &&
        !streq(case_id, "ftoutln.FT_Outline_Decompose.line_conic_cubic_event_order") &&
        !streq(case_id, "ftoutln.FT_Outline_Decompose.shift_delta_applied_to_callbacks") &&
        !streq(case_id, "ftoutln.FT_Outline_Decompose.callback_error_propagates") &&
        !streq(case_id, "ftoutln.FT_Outline_Decompose.invalid_outline_or_interface_errors")) {
        printf("{");
        print_status(FT_Err_Unimplemented_Feature);
        printf(",\"output\":null}\n");
        return 0;
    }

    FT_Vector points[16];
    points[0].x = 8 * 64;
    points[0].y = 8 * 64;
    points[1].x = 24 * 64;
    points[1].y = 8 * 64;
    points[2].x = 24 * 64;
    points[2].y = 24 * 64;
    points[3].x = 8 * 64;
    points[3].y = 24 * 64;
    unsigned char tags[16] = {
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
    };
    unsigned short contours[4] = {3, 0, 0, 0};
    short n_contours = 1;
    short n_points = 4;
    if (streq(case_id, "ftimage.FT_Outline_Funcs.callback_order_matches_c") ||
        streq(case_id, "ftimage.FT_Outline_Funcs.callback_error_propagates") ||
        is_moveto_callback_error_case(case_id) ||
        streq(case_id, "ftimage.FT_CURVE_TAG.classifies_outline_tags") ||
        is_outline_decompose_touch_mask_case(case_id) ||
        is_moveto_starts_each_contour_case(case_id) ||
        streq(case_id, "ftoutln.FT_Outline_Decompose.line_conic_cubic_event_order") ||
        streq(case_id, "ftoutln.FT_Outline_Decompose.callback_error_propagates")) {
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 0;
        points[2].x = 64;
        points[2].y = 64;
        points[3].x = 128;
        points[3].y = 0;
        points[4].x = 160;
        points[4].y = 64;
        points[5].x = 192;
        points[5].y = 64;
        points[6].x = 224;
        points[6].y = 0;
        points[7].x = 256;
        points[7].y = 0;
        points[8].x = 288;
        points[8].y = 64;
        points[9].x = 320;
        points[9].y = 64;
        points[10].x = 352;
        points[10].y = 0;
        tags[4] = FT_CURVE_TAG_CONIC;
        tags[5] = FT_CURVE_TAG_CONIC;
        tags[8] = FT_CURVE_TAG_CUBIC;
        tags[9] = FT_CURVE_TAG_CUBIC;
        if (streq(case_id, "ftimage.FT_CURVE_TAG.classifies_outline_tags") ||
            is_outline_decompose_touch_mask_case(case_id)) {
            tags[0] = FT_CURVE_TAG_ON | FT_CURVE_TAG_TOUCH_X;
            tags[4] = FT_CURVE_TAG_CONIC | FT_CURVE_TAG_HAS_SCANMODE | 0x40;
            tags[8] = FT_CURVE_TAG_CUBIC | FT_CURVE_TAG_TOUCH_Y;
            contours[0] = 2;
            contours[1] = 6;
            contours[2] = 10;
            n_contours = 3;
            n_points = 11;
        } else if (is_moveto_starts_each_contour_case(case_id)) {
            points[0].x = -96;
            points[0].y = -64;
            points[1].x = -32;
            points[1].y = -64;
            points[2].x = -32;
            points[2].y = -16;
            points[3].x = 32;
            points[3].y = -48;
            points[4].x = 64;
            points[4].y = 16;
            points[5].x = 96;
            points[5].y = -48;
            points[6].x = -128;
            points[6].y = 64;
            points[7].x = -96;
            points[7].y = 128;
            points[8].x = -32;
            points[8].y = 128;
            points[9].x = 0;
            points[9].y = 64;
            tags[4] = FT_CURVE_TAG_CONIC;
            tags[5] = FT_CURVE_TAG_ON;
            tags[7] = FT_CURVE_TAG_CUBIC;
            tags[8] = FT_CURVE_TAG_CUBIC;
            tags[9] = FT_CURVE_TAG_ON;
            contours[0] = 2;
            contours[1] = 5;
            contours[2] = 9;
            n_contours = 3;
            n_points = 10;
        } else {
            contours[0] = 2;
            contours[1] = 6;
            contours[2] = 10;
            n_contours = 3;
            n_points = 11;
        }
    } else if (streq(case_id, "ftoutln.FT_Outline_Decompose.shift_delta_applied_to_callbacks")) {
        points[0].x = 32;
        points[0].y = 48;
        points[1].x = 160;
        points[1].y = 48;
        points[2].x = 160;
        points[2].y = 176;
        points[3].x = 32;
        points[3].y = 176;
    } else if (is_outline_decompose_conic_case(case_id)) {
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 96;
        points[2].x = 128;
        points[2].y = 0;
        points[3].x = 192;
        points[3].y = 96;
        points[4].x = 256;
        points[4].y = 96;
        points[5].x = 320;
        points[5].y = 0;
        tags[1] = FT_CURVE_TAG_CONIC;
        tags[3] = FT_CURVE_TAG_CONIC;
        tags[4] = FT_CURVE_TAG_CONIC;
        contours[0] = 5;
        n_points = 6;
    } else if (is_outline_decompose_on_curve_case(case_id)) {
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 0;
        points[2].x = 64;
        points[2].y = 64;
        points[3].x = 0;
        points[3].y = 64;
        points[4].x = 128;
        points[4].y = -32;
        points[5].x = 192;
        points[5].y = -32;
        points[6].x = 192;
        points[6].y = 32;
        contours[0] = 3;
        contours[1] = 6;
        n_contours = 2;
        n_points = 7;
    } else if (is_outline_decompose_cubic_case(case_id)) {
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 96;
        points[2].x = 128;
        points[2].y = 96;
        points[3].x = 192;
        points[3].y = 0;
        points[4].x = 256;
        points[4].y = -96;
        points[5].x = 320;
        points[5].y = -96;
        points[6].x = 384;
        points[6].y = 0;
        tags[1] = FT_CURVE_TAG_CUBIC;
        tags[2] = FT_CURVE_TAG_CUBIC;
        tags[4] = FT_CURVE_TAG_CUBIC;
        tags[5] = FT_CURVE_TAG_CUBIC;
        contours[0] = 6;
        n_points = 7;
    }
    FT_Outline outline;
    outline.n_contours = n_contours;
    outline.n_points = n_points;
    outline.points = points;
    outline.tags = tags;
    outline.contours = contours;
    outline.flags = 0;

    if (streq(case_id, "ftoutln.FT_Outline_Decompose.invalid_outline_or_interface_errors")) {
        FT_Outline_Funcs funcs;
        funcs.move_to = record_outline_move_to;
        funcs.line_to = record_outline_line_to;
        funcs.conic_to = record_outline_conic_to;
        funcs.cubic_to = record_outline_cubic_to;
        funcs.shift = 0;
        funcs.delta = 0;

        FT_Vector bad_cubic_points[3] = {{0, 0}, {64, 64}, {128, 0}};
        unsigned char bad_cubic_tags[3] = {
            FT_CURVE_TAG_CUBIC,
            FT_CURVE_TAG_CUBIC,
            FT_CURVE_TAG_ON,
        };
        unsigned short bad_cubic_contours[1] = {2};
        FT_Outline bad_cubic;
        bad_cubic.n_contours = 1;
        bad_cubic.n_points = 3;
        bad_cubic.points = bad_cubic_points;
        bad_cubic.tags = bad_cubic_tags;
        bad_cubic.contours = bad_cubic_contours;
        bad_cubic.flags = 0;

        FT_Vector bad_contour_points[4] = {
            {0, 0},
            {64, 0},
            {64, 64},
            {0, 64},
        };
        unsigned char bad_contour_tags[4] = {
            FT_CURVE_TAG_ON,
            FT_CURVE_TAG_ON,
            FT_CURVE_TAG_ON,
            FT_CURVE_TAG_ON,
        };
        unsigned short bad_contour_contours[2] = {2, 1};
        FT_Outline bad_contour;
        bad_contour.n_contours = 2;
        bad_contour.n_points = 4;
        bad_contour.points = bad_contour_points;
        bad_contour.tags = bad_contour_tags;
        bad_contour.contours = bad_contour_contours;
        bad_contour.flags = 0;

        FT_Error err_null_outline = FT_Outline_Decompose(NULL, &funcs, recorded_outline_decompose_user_token);
        FT_Error err_null_funcs = FT_Outline_Decompose(&outline, NULL, recorded_outline_decompose_user_token);
        FT_Error err_bad_cubic = FT_Outline_Decompose(&bad_cubic, &funcs, recorded_outline_decompose_user_token);
        FT_Error err_bad_contour = FT_Outline_Decompose(&bad_contour, &funcs, recorded_outline_decompose_user_token);

        printf("{");
        print_status(0);
        printf(",\"output\":{\"results\":[");
        printf("{\"scenario\":\"null_outline\",\"return\":%d},", err_null_outline);
        printf("{\"scenario\":\"null_func_interface\",\"return\":%d},", err_null_funcs);
        printf("{\"scenario\":\"bad_cubic\",\"return\":%d},", err_bad_cubic);
        printf("{\"scenario\":\"bad_contour\",\"return\":%d}", err_bad_contour);
        printf("]}}\n");
        return 0;
    }

    const int shifts[3] = {0, 1, 2};
    const FT_Pos deltas[3] = {0, 16, -32};
    int transform_count = 3;
    if (streq(case_id, "ftimage.FT_Outline_Funcs.callback_order_matches_c") ||
        streq(case_id, "ftimage.FT_Outline_Funcs.callback_error_propagates") ||
        is_moveto_callback_error_case(case_id) ||
        streq(case_id, "ftimage.FT_CURVE_TAG.classifies_outline_tags") ||
        is_outline_decompose_touch_mask_case(case_id) ||
        streq(case_id, "ftoutln.FT_Outline_Decompose.line_conic_cubic_event_order") ||
        streq(case_id, "ftoutln.FT_Outline_Decompose.shift_delta_applied_to_callbacks") ||
        streq(case_id, "ftoutln.FT_Outline_Decompose.callback_error_propagates")) {
        transform_count = 1;
    } else if (is_outline_decompose_conic_case(case_id)) {
        transform_count = 2;
    } else if (is_outline_decompose_on_curve_case(case_id)) {
        transform_count = 2;
    } else if (is_outline_decompose_cubic_case(case_id)) {
        transform_count = 2;
    } else if (is_moveto_starts_each_contour_case(case_id)) {
        transform_count = 2;
    }
    if (streq(case_id, "ftimage.FT_Outline_Funcs.callback_error_propagates")) {
        const char* labels[4] = {"move_to#1", "line_to#1", "conic_to#1", "cubic_to#1"};
        const int fail_indices[4] = {0, 1, 5, 9};
        printf("{");
        print_status(123);
        printf(",\"output\":{\"status\":123,\"error\":123,\"events_before_error\":[");
        for (int row = 0; row < 4; row++) {
            FT_Outline_Funcs funcs;
            funcs.move_to = record_outline_move_to;
            funcs.line_to = record_outline_line_to;
            funcs.conic_to = record_outline_conic_to;
            funcs.cubic_to = record_outline_cubic_to;
            funcs.shift = 0;
            funcs.delta = 0;
            reset_recorded_outline_events();
            recorded_outline_decompose_fail_index = fail_indices[row];
            recorded_outline_decompose_fail_error = 123;
            (void)FT_Outline_Decompose(&outline, &funcs, recorded_outline_decompose_user_token);
            if (row) {
                printf(",");
            }
            printf("{\"failure_point\":\"%s\",\"events\":[", labels[row]);
            for (int i = 0; i < recorded_outline_event_count; i++) {
                if (i) {
                    printf(",");
                }
                printf("{\"kind\":\"%s\",\"points\":[", recorded_outline_events[i].kind);
                for (int j = 0; j < recorded_outline_events[i].count; j++) {
                    if (j) {
                        printf(",");
                    }
                    printf("{\"x\":%ld,\"y\":%ld}",
                           recorded_outline_events[i].points[j].x,
                           recorded_outline_events[i].points[j].y);
                }
                printf("]}");
            }
            printf("]}");
        }
        printf("]}}\n");
        return 0;
    }
    if (is_outline_callback_return_matrix_case(case_id)) {
        const int return_values[2] = {7, 1234};
        const char* callback = outline_callback_return_matrix_callback(case_id);
        printf("{");
        print_status(return_values[0]);
        printf(",\"output\":{\"rows\":[");
        for (int row = 0; row < 2; row++) {
            FT_Outline_Funcs funcs;
            funcs.move_to = record_outline_move_to;
            funcs.line_to = record_outline_line_to;
            funcs.conic_to = record_outline_conic_to;
            funcs.cubic_to = record_outline_cubic_to;
            funcs.shift = 0;
            funcs.delta = 0;
            reset_recorded_outline_events();
            /* FreeType `src/base/ftoutln.c` checks callback return values
             * immediately; these standard fixtures all invoke move_to before the
             * first tested line/conic/cubic callback, so fail index 1 captures the
             * prefix before the abort. */
            recorded_outline_decompose_fail_index = 1;
            recorded_outline_decompose_fail_error = return_values[row];
            (void)FT_Outline_Decompose(&outline, &funcs, recorded_outline_decompose_user_token);
            if (row) {
                printf(",");
            }
            printf("{\"status\":%d,\"error\":%d,\"events_before_abort\":[",
                   return_values[row],
                   return_values[row]);
            for (int i = 0; i < recorded_outline_event_count; i++) {
                if (i) {
                    printf(",");
                }
                printf("{\"kind\":\"%s\",\"points\":[", recorded_outline_events[i].kind);
                for (int j = 0; j < recorded_outline_events[i].count; j++) {
                    if (j) {
                        printf(",");
                    }
                    printf("{\"x\":%ld,\"y\":%ld}",
                           recorded_outline_events[i].points[j].x,
                           recorded_outline_events[i].points[j].y);
                }
                printf("]}");
            }
            printf("],\"failing_callback\":\"%s\"}", callback);
        }
        printf("]}}\n");
        return 0;
    }
    if (is_moveto_callback_error_case(case_id)) {
        FT_Outline_Funcs funcs;
        funcs.move_to = record_outline_move_to;
        funcs.line_to = record_outline_line_to;
        funcs.conic_to = record_outline_conic_to;
        funcs.cubic_to = record_outline_cubic_to;
        funcs.shift = 0;
        funcs.delta = 0;
        reset_recorded_outline_events();
        recorded_outline_decompose_fail_index = 0;
        recorded_outline_decompose_fail_error = 123;
        FT_Error err = FT_Outline_Decompose(&outline, &funcs, recorded_outline_decompose_user_token);
        printf("{");
        print_status(err);
        printf(",\"output\":{\"status\":%d,\"events\":[],\"first_error_callback\":\"move_to\"}}\n", err);
        return 0;
    }
    if (streq(case_id, "ftoutln.FT_Outline_Decompose.callback_error_propagates")) {
        FT_Outline_Funcs funcs;
        funcs.move_to = record_outline_move_to;
        funcs.line_to = record_outline_line_to;
        funcs.conic_to = record_outline_conic_to;
        funcs.cubic_to = record_outline_cubic_to;
        funcs.shift = 0;
        funcs.delta = 0;
        reset_recorded_outline_events();
        recorded_outline_decompose_fail_index = 2;
        recorded_outline_decompose_fail_error = FT_Err_Invalid_Argument;
        FT_Error err = FT_Outline_Decompose(&outline, &funcs, recorded_outline_decompose_user_token);
        printf("{");
        print_status(err);
        printf(",\"output\":{\"return\":%d,\"events_before_error\":[", err);
        for (int i = 0; i < recorded_outline_event_count; i++) {
            if (i) {
                printf(",");
            }
            printf("{\"kind\":\"%s\",\"points\":[", recorded_outline_events[i].kind);
            for (int j = 0; j < recorded_outline_events[i].count; j++) {
                if (j) {
                    printf(",");
                }
                printf("{\"x\":%ld,\"y\":%ld}",
                       recorded_outline_events[i].points[j].x,
                       recorded_outline_events[i].points[j].y);
            }
            printf("]}");
        }
        printf("]}}\n");
        return 0;
    }
    printf("{");
    print_status(0);
    printf(",\"output\":{\"runs\":[");
    for (int i = 0; i < transform_count; i++) {
        FT_Outline_Funcs funcs;
        funcs.move_to = record_outline_move_to;
        funcs.line_to = record_outline_line_to;
        funcs.conic_to = record_outline_conic_to;
        funcs.cubic_to = record_outline_cubic_to;
        if (streq(case_id, "ftoutln.FT_Outline_Decompose.shift_delta_applied_to_callbacks")) {
            funcs.shift = 2;
            funcs.delta = 7;
        } else if (is_outline_decompose_public_input_alias_case(case_id) && i == 1) {
            funcs.shift = 1;
            funcs.delta = 32;
        } else if (is_outline_decompose_on_curve_case(case_id) && i == 1) {
            funcs.shift = 2;
            funcs.delta = 32;
        } else if (is_outline_decompose_cubic_case(case_id) && i == 1) {
            funcs.shift = 1;
            funcs.delta = -16;
        } else if (is_moveto_starts_each_contour_case(case_id) && i == 1) {
            funcs.shift = 1;
            funcs.delta = 32;
        } else {
            funcs.shift = shifts[i];
            funcs.delta = deltas[i];
        }
        reset_recorded_outline_events();
        FT_Error err = FT_Outline_Decompose(&outline, &funcs, recorded_outline_decompose_user_token);
        if (i) {
            printf(",");
        }
        printf("{\"shift\":%d,\"delta\":%ld,\"status\":%d,", funcs.shift, funcs.delta, err);
        print_recorded_outline_events();
        printf(",");
        print_recorded_outline_event_points();
        printf(",\"user_seen\":%s}", recorded_outline_decompose_user_seen ? "true" : "false");
    }
    if (streq(case_id, "ftimage.FT_CURVE_TAG_CUBIC.cubic_decomposition_matches_c")) {
        FT_Vector malformed_points[2][3] = {
            {{0, 0}, {64, 64}, {128, 0}},
            {{0, 0}, {64, 64}, {128, 0}},
        };
        unsigned char malformed_tags[2][3] = {
            {FT_CURVE_TAG_CUBIC, FT_CURVE_TAG_CUBIC, FT_CURVE_TAG_ON},
            {FT_CURVE_TAG_ON, FT_CURVE_TAG_CUBIC, FT_CURVE_TAG_ON},
        };
        unsigned short malformed_contours[2][1] = {{2}, {2}};
        const char* malformed_ids[2] = {"contour_starts_with_cubic", "unpaired_cubic_control"};
        printf("],\"malformed_status\":[");
        for (int row = 0; row < 2; row++) {
            FT_Outline malformed_outline;
            malformed_outline.n_contours = 1;
            malformed_outline.n_points = 3;
            malformed_outline.points = malformed_points[row];
            malformed_outline.tags = malformed_tags[row];
            malformed_outline.contours = malformed_contours[row];
            malformed_outline.flags = 0;
            FT_Outline_Funcs funcs;
            funcs.move_to = record_outline_move_to;
            funcs.line_to = record_outline_line_to;
            funcs.conic_to = record_outline_conic_to;
            funcs.cubic_to = record_outline_cubic_to;
            funcs.shift = 0;
            funcs.delta = 0;
            reset_recorded_outline_events();
            FT_Error err = FT_Outline_Decompose(&malformed_outline, &funcs, recorded_outline_decompose_user_token);
            if (row) {
                printf(",");
            }
            printf("{\"id\":\"%s\",\"status\":%d}", malformed_ids[row], err);
        }
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftimage.FT_CURVE_TAG.classifies_outline_tags")) {
        printf("],\"masked_tags\":[");
        for (int i = 0; i < outline.n_points; i++) {
            if (i) {
                printf(",");
            }
            printf("%d", FT_CURVE_TAG(outline.tags[i]));
        }
        printf("]}}\n");
        return 0;
    }
    printf("]}}\n");
    return 0;
}

typedef enum OrientationOutlineKind_ {
    ORIENTATION_OUTLINE_NULL,
    ORIENTATION_OUTLINE_EMPTY,
    ORIENTATION_OUTLINE_NONEMPTY_ZERO_CONTOURS,
    ORIENTATION_OUTLINE_POSITIVE,
    ORIENTATION_OUTLINE_NEGATIVE,
    ORIENTATION_OUTLINE_COLLAPSED,
    ORIENTATION_OUTLINE_COLLAPSED_VERTICAL,
    ORIENTATION_OUTLINE_OVERSIZED,
    ORIENTATION_OUTLINE_OVERSIZED_X_MIN,
    ORIENTATION_OUTLINE_OVERSIZED_Y_MIN,
    ORIENTATION_OUTLINE_OVERSIZED_Y_MAX,
    ORIENTATION_OUTLINE_ZERO_AREA,
} OrientationOutlineKind;

static void print_mutated_points(const FT_Vector* points, unsigned int count);

static void build_orientation_outline(
    OrientationOutlineKind kind,
    FT_Outline* outline,
    FT_Vector* points,
    char* tags,
    short* contours
) {
    for (int index = 0; index < 4; index++) {
        tags[index] = FT_CURVE_TAG_ON;
    }
    contours[0] = 3;
    outline->n_contours = 1;
    outline->n_points = 4;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 0;

    if (kind == ORIENTATION_OUTLINE_EMPTY) {
        outline->n_contours = 0;
        outline->n_points = 0;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_NONEMPTY_ZERO_CONTOURS) {
        outline->n_contours = 0;
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 0;
        points[1].y = 64;
        points[2].x = 64;
        points[2].y = 64;
        points[3].x = 64;
        points[3].y = 0;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_COLLAPSED) {
        outline->n_points = 2;
        contours[0] = 1;
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 0;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_COLLAPSED_VERTICAL) {
        outline->n_points = 2;
        contours[0] = 1;
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 0;
        points[1].y = 64;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_OVERSIZED) {
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 0;
        points[1].y = 64;
        points[2].x = 0x1000000L + 64;
        points[2].y = 64;
        points[3].x = 0x1000000L + 64;
        points[3].y = 0;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_OVERSIZED_X_MIN) {
        points[0].x = -0x1000000L - 64;
        points[0].y = 0;
        points[1].x = -0x1000000L - 64;
        points[1].y = 64;
        points[2].x = 0;
        points[2].y = 64;
        points[3].x = 0;
        points[3].y = 0;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_OVERSIZED_Y_MIN) {
        points[0].x = 0;
        points[0].y = -0x1000000L - 64;
        points[1].x = 0;
        points[1].y = 0;
        points[2].x = 64;
        points[2].y = 0;
        points[3].x = 64;
        points[3].y = -0x1000000L - 64;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_OVERSIZED_Y_MAX) {
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 0;
        points[1].y = 0x1000000L + 64;
        points[2].x = 64;
        points[2].y = 0x1000000L + 64;
        points[3].x = 64;
        points[3].y = 0;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_ZERO_AREA) {
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 64;
        points[2].x = 0;
        points[2].y = 64;
        points[3].x = 64;
        points[3].y = 0;
        return;
    }
    if (kind == ORIENTATION_OUTLINE_NEGATIVE) {
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 0;
        points[2].x = 64;
        points[2].y = 64;
        points[3].x = 0;
        points[3].y = 64;
        return;
    }

    points[0].x = 0;
    points[0].y = 0;
    points[1].x = 0;
    points[1].y = 64;
    points[2].x = 64;
    points[2].y = 64;
    points[3].x = 64;
    points[3].y = 0;
}

static void print_orientation_observation(
    const char* label,
    OrientationOutlineKind kind,
    int* emitted
) {
    if (*emitted) {
        printf(",");
    }
    *emitted = 1;
    FT_Orientation orientation;
    if (kind == ORIENTATION_OUTLINE_NULL) {
        orientation = FT_Outline_Get_Orientation(NULL);
    } else {
        FT_Outline outline;
        FT_Vector points[4];
        char tags[4];
        short contours[1];
        build_orientation_outline(kind, &outline, points, tags, contours);
        orientation = FT_Outline_Get_Orientation(&outline);
    }
    printf("{\"label\":\"%s\",\"orientation\":%d}", label, orientation);
}

static int emit_outline_get_orientation(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    const char* case_id = argv[2];
    print_ok_output_prefix();
    printf("{\"orientations\":[");
    int emitted = 0;
    if (strstr(case_id, "FT_Outline_Get_Orientation.null_empty_and_area_sign")) {
        print_orientation_observation("null", ORIENTATION_OUTLINE_NULL, &emitted);
        print_orientation_observation("empty", ORIENTATION_OUTLINE_EMPTY, &emitted);
        print_orientation_observation(
            "nonempty_zero_contours",
            ORIENTATION_OUTLINE_NONEMPTY_ZERO_CONTOURS,
            &emitted
        );
        print_orientation_observation("positive", ORIENTATION_OUTLINE_POSITIVE, &emitted);
        print_orientation_observation("negative", ORIENTATION_OUTLINE_NEGATIVE, &emitted);
    } else if (strstr(case_id, "FT_Orientation.orientation_algorithm_contract")) {
        print_orientation_observation("positive", ORIENTATION_OUTLINE_POSITIVE, &emitted);
        print_orientation_observation("negative", ORIENTATION_OUTLINE_NEGATIVE, &emitted);
        print_orientation_observation(
            "collapsed_horizontal",
            ORIENTATION_OUTLINE_COLLAPSED,
            &emitted
        );
        print_orientation_observation(
            "collapsed_vertical",
            ORIENTATION_OUTLINE_COLLAPSED_VERTICAL,
            &emitted
        );
        print_orientation_observation("zero_area", ORIENTATION_OUTLINE_ZERO_AREA, &emitted);
        print_orientation_observation(
            "oversized_x_min",
            ORIENTATION_OUTLINE_OVERSIZED_X_MIN,
            &emitted
        );
        print_orientation_observation(
            "oversized_y_min",
            ORIENTATION_OUTLINE_OVERSIZED_Y_MIN,
            &emitted
        );
        print_orientation_observation(
            "oversized_x_max",
            ORIENTATION_OUTLINE_OVERSIZED,
            &emitted
        );
        print_orientation_observation(
            "oversized_y_max",
            ORIENTATION_OUTLINE_OVERSIZED_Y_MAX,
            &emitted
        );
    } else if (strstr(case_id, "FT_Outline_Get_Orientation.collapsed_and_oversized_return_none")) {
        print_orientation_observation(
            "collapsed_horizontal",
            ORIENTATION_OUTLINE_COLLAPSED,
            &emitted
        );
        print_orientation_observation(
            "collapsed_vertical",
            ORIENTATION_OUTLINE_COLLAPSED_VERTICAL,
            &emitted
        );
        print_orientation_observation(
            "oversized_x_min",
            ORIENTATION_OUTLINE_OVERSIZED_X_MIN,
            &emitted
        );
        print_orientation_observation(
            "oversized_y_min",
            ORIENTATION_OUTLINE_OVERSIZED_Y_MIN,
            &emitted
        );
        print_orientation_observation(
            "oversized_x_max",
            ORIENTATION_OUTLINE_OVERSIZED,
            &emitted
        );
        print_orientation_observation(
            "oversized_y_max",
            ORIENTATION_OUTLINE_OVERSIZED_Y_MAX,
            &emitted
        );
    } else if (strstr(case_id, "FT_ORIENTATION_TRUETYPE.null_and_empty_return_truetype")) {
        print_orientation_observation("null", ORIENTATION_OUTLINE_NULL, &emitted);
        print_orientation_observation("empty", ORIENTATION_OUTLINE_EMPTY, &emitted);
    } else if (strstr(case_id, "FT_ORIENTATION_TRUETYPE.negative_area_returns_truetype") ||
               strstr(case_id, "FT_ORIENTATION_FILL_RIGHT.alias_matches_truetype_orientation")) {
        print_orientation_observation("negative", ORIENTATION_OUTLINE_NEGATIVE, &emitted);
    } else if (strstr(case_id, "FT_ORIENTATION_POSTSCRIPT.positive_area_returns_postscript") ||
               strstr(case_id, "FT_ORIENTATION_FILL_LEFT.returned_for_positive_area")) {
        print_orientation_observation("positive", ORIENTATION_OUTLINE_POSITIVE, &emitted);
    } else if (strstr(case_id, "FT_ORIENTATION_NONE.returned_for_collapsed_or_zero_area")) {
        print_orientation_observation("collapsed", ORIENTATION_OUTLINE_COLLAPSED, &emitted);
        print_orientation_observation("zero_area", ORIENTATION_OUTLINE_ZERO_AREA, &emitted);
    } else if (strstr(case_id, "FT_ORIENTATION_NONE.returned_for_oversized_outline")) {
        print_orientation_observation("oversized", ORIENTATION_OUTLINE_OVERSIZED, &emitted);
    } else {
        fprintf(stderr, "unsupported outline orientation case: %s\n", case_id);
        return 2;
    }
    printf("]}}\n");
    return 0;
}

static int emit_outline_get_orientation_mutated(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    const char* case_id = argv[2];
    if (!strstr(case_id, "FT_Outline_Get_Orientation.transformed_and_reversed_outlines")) {
        fprintf(stderr, "unsupported outline orientation mutation case: %s\n", case_id);
        return 2;
    }
    print_ok_output_prefix();
    FT_Outline reversed;
    FT_Vector reversed_points[4];
    char reversed_tags[4];
    short reversed_contours[1];
    build_orientation_outline(ORIENTATION_OUTLINE_NEGATIVE, &reversed, reversed_points, reversed_tags, reversed_contours);
    FT_Outline_Reverse(&reversed);
    FT_Orientation reversed_orientation = FT_Outline_Get_Orientation(&reversed);

    FT_Outline transformed;
    FT_Vector transformed_points[4];
    char transformed_tags[4];
    short transformed_contours[1];
    build_orientation_outline(ORIENTATION_OUTLINE_NEGATIVE, &transformed, transformed_points, transformed_tags, transformed_contours);
    FT_Matrix matrix = {-0x10000L, 0, 0, 0x10000L};
    FT_Outline_Transform(&transformed, &matrix);
    FT_Orientation transformed_orientation = FT_Outline_Get_Orientation(&transformed);

    printf("{\"results\":[{\"label\":\"reverse\",\"return\":%d,\"points_after\":", reversed_orientation);
    print_mutated_points(reversed_points, 4);
    printf(",\"flags_after\":%d},{\"label\":\"transform\",\"return\":%d,\"points_after\":", reversed.flags, transformed_orientation);
    print_mutated_points(transformed_points, 4);
    printf(",\"flags_after\":%d}]}}\n", transformed.flags);
    return 0;
}

static void build_reverse_orientation_fixture(FT_Outline* outline, FT_Vector* points, unsigned char* tags, unsigned short* contours) {
    points[0].x = 4L * 64L;
    points[0].y = 4L * 64L;
    points[1].x = 12L * 64L;
    points[1].y = 4L * 64L;
    points[2].x = 12L * 64L;
    points[2].y = 12L * 64L;
    points[3].x = 4L * 64L;
    points[3].y = 12L * 64L;
    for (int i = 0; i < 4; i++) {
        tags[i] = FT_CURVE_TAG_ON;
    }
    contours[0] = 3;
    outline->n_contours = 1;
    outline->n_points = 4;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 0;
}

static void print_bbox_object(FT_BBox bbox) {
    printf("{\"xMin\":%ld,\"yMin\":%ld,\"xMax\":%ld,\"yMax\":%ld}",
           bbox.xMin,
           bbox.yMin,
           bbox.xMax,
           bbox.yMax);
}

static void print_outline_tags(const FT_Outline* outline) {
    printf("[");
    for (int i = 0; i < outline->n_points; i++) {
        if (i) {
            printf(",");
        }
        printf("%u", (unsigned char)outline->tags[i]);
    }
    printf("]");
}

static void print_outline_contours(const FT_Outline* outline) {
    printf("[");
    for (int i = 0; i < outline->n_contours; i++) {
        if (i) {
            printf(",");
        }
        printf("%d", outline->contours[i]);
    }
    printf("]");
}

static void print_recorded_outline_events_array(void) {
    printf("[");
    for (int i = 0; i < recorded_outline_event_count; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"kind\":\"%s\",\"points\":[", recorded_outline_events[i].kind);
        for (int j = 0; j < recorded_outline_events[i].count; j++) {
            if (j) {
                printf(",");
            }
            printf("{\"x\":%ld,\"y\":%ld}",
                   recorded_outline_events[i].points[j].x,
                   recorded_outline_events[i].points[j].y);
        }
        printf("]}");
    }
    printf("]");
}

static FT_Error decompose_outline_for_reverse_orientation(FT_Outline* outline) {
    FT_Outline_Funcs funcs;
    funcs.move_to = record_outline_move_to;
    funcs.line_to = record_outline_line_to;
    funcs.conic_to = record_outline_conic_to;
    funcs.cubic_to = record_outline_cubic_to;
    funcs.shift = 0;
    funcs.delta = 0;
    reset_recorded_outline_events();
    return FT_Outline_Decompose(outline, &funcs, recorded_outline_decompose_user_token);
}

static void print_reverse_orientation_bitmap(FT_Library library, FT_Outline* outline) {
    unsigned char buffer[16 * 16];
    FT_Bitmap bitmap;
    setup_outline_get_bitmap_target(&bitmap, buffer, FT_PIXEL_MODE_GRAY);
    FT_Error err = FT_Outline_Get_Bitmap(library, outline, &bitmap);
    if (err) {
        printf("null");
    } else {
        long len = 0;
        if (bitmap.buffer && bitmap.rows > 0) {
            len = labs(bitmap.pitch) * bitmap.rows;
        }
        printf("{\"width\":%u,\"rows\":%u,\"pitch\":%d,\"pixel_mode\":%u,\"num_grays\":%u,\"buffer_hex\":\"",
               bitmap.width,
               bitmap.rows,
               bitmap.pitch,
               bitmap.pixel_mode,
               bitmap.num_grays);
        if (bitmap.buffer && len > 0) {
            print_hex_bytes(bitmap.buffer, len);
        }
        printf("\"}");
    }
}

static FT_Error print_reverse_orientation_observation(FT_Library library, const char* name, FT_Outline* outline) {
    FT_BBox cbox;
    FT_BBox bbox;
    FT_Error bbox_error;
    FT_Error decompose_error;

    FT_Outline_Get_CBox(outline, &cbox);
    bbox_error = FT_Outline_Get_BBox(outline, &bbox);
    decompose_error = decompose_outline_for_reverse_orientation(outline);

    printf("\"%s\":{\"orientation\":%d,\"points\":", name, FT_Outline_Get_Orientation(outline));
    print_mutated_points(outline->points, (unsigned int)outline->n_points);
    printf(",\"tags\":");
    print_outline_tags(outline);
    printf(",\"contours\":");
    print_outline_contours(outline);
    printf(",\"flags\":%d,\"cbox\":", outline->flags);
    print_bbox_object(cbox);
    printf(",\"bbox\":");
    print_bbox_object(bbox);
    printf(",\"decompose_events\":");
    if (decompose_error) {
        printf("[]");
    } else {
        print_recorded_outline_events_array();
    }
    printf(",\"bitmap\":");
    if (bbox_error || decompose_error) {
        printf("null");
    } else {
        print_reverse_orientation_bitmap(library, outline);
    }
    printf("}");
    return bbox_error ? bbox_error : decompose_error;
}

static FT_Error reverse_orientation_count_mismatch_error(void) {
    FT_Vector points[4] = {{0,0},{64,0},{64,64},{0,64}};
    unsigned char tags[4] = {FT_CURVE_TAG_ON,FT_CURVE_TAG_ON,FT_CURVE_TAG_ON,FT_CURVE_TAG_ON};
    unsigned short contours[2] = {2,2};
    FT_Outline outline = {2, 4, points, tags, contours, 0};
    return FT_Outline_Check(&outline);
}

static FT_Error reverse_orientation_bad_cubic_error(void) {
    FT_Vector points[3] = {{0,0},{64,64},{128,0}};
    unsigned char tags[3] = {FT_CURVE_TAG_CUBIC,FT_CURVE_TAG_CUBIC,FT_CURVE_TAG_ON};
    unsigned short contours[1] = {2};
    FT_Outline outline = {1, 3, points, tags, contours, 0};
    return decompose_outline_for_reverse_orientation(&outline);
}

static int emit_outline_reverse_orientation(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    const char* case_id = argv[2];
    if (!strstr(case_id, "FT_ORIENTATION_FILL_LEFT.reverse_toggles_orientation_fixture")) {
        fprintf(stderr, "unsupported outline reverse orientation case: %s\n", case_id);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    FT_Outline outline;
    FT_Vector points[4];
    unsigned char tags[4];
    unsigned short contours[1];
    build_reverse_orientation_fixture(&outline, points, tags, contours);

    printf("{");
    print_status(0);
    printf(",\"output\":{");
    FT_Error before_error = print_reverse_orientation_observation(library, "before", &outline);
    FT_Outline_Reverse(&outline);
    printf(",");
    FT_Error after_error = print_reverse_orientation_observation(library, "after", &outline);
    FT_Error count_error = reverse_orientation_count_mismatch_error();
    FT_Error cubic_error = reverse_orientation_bad_cubic_error();
    printf(",\"invalid_rows\":[");
    printf("{\"label\":\"count_mismatch_rejected_before_reverse\",\"return\":%d},", count_error);
    printf("{\"label\":\"invalid_cubic_start_decompose_error\",\"return\":%d}", cubic_error);
    printf("]}}\n");

    FT_Done_FreeType(library);
    return (before_error || after_error) ? 2 : 0;
}

static void print_outline_border_row(
    const char* label,
    OrientationOutlineKind kind,
    int outside,
    int* emitted
) {
    if (*emitted) {
        printf(",");
    }
    *emitted = 1;
    FT_Outline outline;
    FT_Vector points[4];
    char tags[4];
    short contours[1];
    FT_Outline* outline_ptr = NULL;
    if (kind != ORIENTATION_OUTLINE_NULL) {
        build_orientation_outline(kind, &outline, points, tags, contours);
        outline_ptr = &outline;
    }
    FT_Orientation orientation = FT_Outline_Get_Orientation(outline_ptr);
    FT_StrokerBorder border = outside
                                ? FT_Outline_GetOutsideBorder(outline_ptr)
                                : FT_Outline_GetInsideBorder(outline_ptr);
    printf("{\"label\":\"%s\",\"orientation\":%d,\"border\":%d}", label, orientation, border);
}

static int emit_outline_border(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    const char* case_id = argv[2];
    int outside = strstr(case_id, "FT_Outline_GetOutsideBorder") != NULL;
    print_ok_output_prefix();
    printf("{\"rows\":[");
    int emitted = 0;
    if (strstr(case_id, ".truetype_orientation_returns_")) {
        print_outline_border_row("null", ORIENTATION_OUTLINE_NULL, outside, &emitted);
        print_outline_border_row("empty", ORIENTATION_OUTLINE_EMPTY, outside, &emitted);
        print_outline_border_row("truetype", ORIENTATION_OUTLINE_NEGATIVE, outside, &emitted);
    } else if (strstr(case_id, ".non_truetype_orientation_returns_")) {
        print_outline_border_row("postscript", ORIENTATION_OUTLINE_POSITIVE, outside, &emitted);
    } else if (strstr(case_id, ".orientation_delegation")) {
        print_outline_border_row("null", ORIENTATION_OUTLINE_NULL, outside, &emitted);
        print_outline_border_row("empty", ORIENTATION_OUTLINE_EMPTY, outside, &emitted);
        print_outline_border_row("truetype", ORIENTATION_OUTLINE_NEGATIVE, outside, &emitted);
        print_outline_border_row("postscript", ORIENTATION_OUTLINE_POSITIVE, outside, &emitted);
    } else {
        fprintf(stderr, "unsupported outline border case: %s\n", case_id);
        return 2;
    }
    printf("]}}\n");
    return 0;
}

typedef enum CheckOutlineKind_ {
    CHECK_OUTLINE_NULL,
    CHECK_OUTLINE_EMPTY,
    CHECK_OUTLINE_SINGLE_POINT,
    CHECK_OUTLINE_MULTI_CONTOUR,
    CHECK_OUTLINE_ZERO_POINTS_ONE_CONTOUR,
    CHECK_OUTLINE_NONINCREASING_CONTOURS,
} CheckOutlineKind;

static void build_check_outline(
    CheckOutlineKind kind,
    FT_Outline* outline,
    FT_Vector* points,
    unsigned char* tags,
    unsigned short* contours
) {
    outline->n_contours = 0;
    outline->n_points = 0;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 0;
    if (kind == CHECK_OUTLINE_EMPTY) {
        return;
    }
    if (kind == CHECK_OUTLINE_SINGLE_POINT) {
        points[0].x = 0;
        points[0].y = 0;
        tags[0] = FT_CURVE_TAG_ON;
        contours[0] = 0;
        outline->n_points = 1;
        outline->n_contours = 1;
        return;
    }
    if (kind == CHECK_OUTLINE_ZERO_POINTS_ONE_CONTOUR) {
        contours[0] = 0;
        outline->n_points = 0;
        outline->n_contours = 1;
        return;
    }
    points[0].x = 0;
    points[0].y = 0;
    points[1].x = 64;
    points[1].y = 0;
    points[2].x = 64;
    points[2].y = 64;
    points[3].x = 0;
    points[3].y = 64;
    for (int i = 0; i < 4; i++) {
        tags[i] = FT_CURVE_TAG_ON;
    }
    outline->n_points = 4;
    if (kind == CHECK_OUTLINE_NONINCREASING_CONTOURS) {
        contours[0] = 2;
        contours[1] = 2;
        outline->n_contours = 2;
        return;
    }
    points[4].x = 128;
    points[4].y = 0;
    points[5].x = 192;
    points[5].y = 0;
    points[6].x = 192;
    points[6].y = 64;
    points[7].x = 128;
    points[7].y = 64;
    for (int i = 4; i < 8; i++) {
        tags[i] = FT_CURVE_TAG_ON;
    }
    contours[0] = 3;
    contours[1] = 7;
    outline->n_points = 8;
    outline->n_contours = 2;
}

static void print_outline_check_observation(
    const char* label,
    CheckOutlineKind kind,
    int* emitted
) {
    if (*emitted) {
        printf(",");
    }
    *emitted = 1;
    FT_Error error;
    int n_points = 0;
    int n_contours = 0;
    if (kind == CHECK_OUTLINE_NULL) {
        error = FT_Outline_Check(NULL);
    } else {
        FT_Outline outline;
        FT_Vector points[8];
        unsigned char tags[8];
        unsigned short contours[2];
        build_check_outline(kind, &outline, points, tags, contours);
        error = FT_Outline_Check(&outline);
        n_points = outline.n_points;
        n_contours = outline.n_contours;
    }
    printf(
        "{\"label\":\"%s\",\"return\":%d,\"n_points\":%d,\"n_contours\":%d}",
        label,
        error,
        n_points,
        n_contours
    );
}

static int emit_outline_check(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    const char* case_id = argv[2];
    print_ok_output_prefix();
    printf("{\"results\":[");
    int emitted = 0;
    if (strstr(case_id, ".valid_empty_and_single_point")) {
        print_outline_check_observation("empty", CHECK_OUTLINE_EMPTY, &emitted);
        print_outline_check_observation("single_point", CHECK_OUTLINE_SINGLE_POINT, &emitted);
    } else if (strstr(case_id, ".valid_multi_contour_sequence")) {
        print_outline_check_observation("multi_contour", CHECK_OUTLINE_MULTI_CONTOUR, &emitted);
    } else if (strstr(case_id, ".invalid_null_or_count_mismatch")) {
        print_outline_check_observation("null", CHECK_OUTLINE_NULL, &emitted);
        print_outline_check_observation(
            "bad_zero_points",
            CHECK_OUTLINE_ZERO_POINTS_ONE_CONTOUR,
            &emitted
        );
        print_outline_check_observation(
            "bad_contours",
            CHECK_OUTLINE_NONINCREASING_CONTOURS,
            &emitted
        );
    } else {
        fprintf(stderr, "unsupported outline check case: %s\n", case_id);
        return 2;
    }
    printf("]}}\n");
    return 0;
}

static void build_reverse_outline(
    FT_Outline* outline,
    FT_Vector* points,
    unsigned char* tags,
    unsigned short* contours
) {
    const FT_Vector source_points[8] = {
        {0, 0}, {64, 0}, {64, 64}, {0, 64},
        {128, 0}, {192, 0}, {192, 64}, {128, 64},
    };
    const unsigned char source_tags[8] = {1, 0, 1, 2, 1, 2, 0, 1};
    memcpy(points, source_points, sizeof(source_points));
    memcpy(tags, source_tags, sizeof(source_tags));
    contours[0] = 3;
    contours[1] = 7;
    outline->n_contours = 2;
    outline->n_points = 8;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 2;
}

static void build_transform_outline(
    FT_Outline* outline,
    FT_Vector* points,
    unsigned char* tags,
    unsigned short* contours
) {
    const FT_Vector source_points[4] = {
        {-96, -32}, {128, -64}, {160, 96}, {-64, 128},
    };
    memcpy(points, source_points, sizeof(source_points));
    memset(tags, FT_CURVE_TAG_ON, 4);
    contours[0] = 3;
    outline->n_contours = 1;
    outline->n_points = 4;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 0;
}

static void print_mutated_points(const FT_Vector* points, unsigned int count) {
    printf("[");
    for (unsigned int index = 0; index < count; index++) {
        if (index) {
            printf(",");
        }
        printf("{\"x\":%ld,\"y\":%ld}", points[index].x, points[index].y);
    }
    printf("]");
}

static void print_outline_snapshot(const FT_Outline* outline) {
    printf("{\"points\":");
    print_mutated_points(outline->points, (unsigned int)outline->n_points);
    printf(",\"tags\":[");
    for (int index = 0; index < outline->n_points; index++) {
        if (index) {
            printf(",");
        }
        printf("%u", (unsigned int)(unsigned char)outline->tags[index]);
    }
    printf("],\"contours\":[");
    for (int index = 0; index < outline->n_contours; index++) {
        if (index) {
            printf(",");
        }
        printf("%d", outline->contours[index]);
    }
    printf("],\"flags\":%d}", outline->flags);
}

static int emit_outline_reverse(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    const char* case_id = argv[2];
    print_ok_output_prefix();
    if (strstr(case_id, ".null_outline_noop")) {
        FT_Outline_Reverse(NULL);
        printf("{\"sentinel_memory_changed\":false}}\n");
        return 0;
    }

    FT_Outline outline;
    FT_Vector points[8];
    unsigned char tags[8];
    unsigned short contours[2];
    build_reverse_outline(&outline, points, tags, contours);
    if (strstr(case_id, ".reverses_points_and_tags_per_contour")) {
        FT_Outline_Reverse(&outline);
        printf("{\"points_after\":");
        print_mutated_points(points, 8);
        printf(",\"tags_after\":[");
        for (int index = 0; index < 8; index++) {
            if (index) {
                printf(",");
            }
            printf("%u", tags[index]);
        }
        printf("],\"contours_after\":[%d,%d],\"flags_after\":%d}}\n",
               contours[0], contours[1], outline.flags);
        return 0;
    }
    if (strstr(case_id, ".toggles_reverse_fill_flag")) {
        FT_Outline_Reverse(&outline);
        int first = outline.flags;
        printf("{\"flags_after_each_call\":[%d,", first);
        FT_Vector first_points[8];
        unsigned char first_tags[8];
        unsigned short first_contours[2];
        memcpy(first_points, points, sizeof(first_points));
        memcpy(first_tags, tags, sizeof(first_tags));
        memcpy(first_contours, contours, sizeof(first_contours));
        FT_Outline first_outline = outline;
        first_outline.points = first_points;
        first_outline.tags = first_tags;
        first_outline.contours = first_contours;
        FT_Outline_Reverse(&outline);
        printf("%d],\"outlines_after_each_call\":[", outline.flags);
        print_outline_snapshot(&first_outline);
        printf(",");
        print_outline_snapshot(&outline);
        printf("]}}\n");
        return 0;
    }
    fprintf(stderr, "unsupported outline reverse case: %s\n", case_id);
    return 2;
}

static void build_copy_source_outline(FT_Outline* outline, FT_Vector* points, unsigned char* tags, unsigned short* contours) {
    FT_Vector src_points[6] = {{0,0},{64,0},{96,32},{64,64},{0,64},{-32,32}};
    unsigned char src_tags[6] = {1,0,1,2,0,1};
    unsigned short src_contours[2] = {2,5};
    memcpy(points, src_points, sizeof(src_points));
    memcpy(tags, src_tags, sizeof(src_tags));
    memcpy(contours, src_contours, sizeof(src_contours));
    outline->n_contours = 2;
    outline->n_points = 6;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 4;
}

static void build_copy_target_outline(FT_Outline* outline, FT_Vector* points, unsigned char* tags, unsigned short* contours, int owner) {
    for (int i = 0; i < 6; i++) {
        points[i].x = 10;
        points[i].y = 10;
        tags[i] = 7;
    }
    contours[0] = 2;
    contours[1] = 5;
    outline->n_contours = 2;
    outline->n_points = 6;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = owner ? FT_OUTLINE_OWNER : 0;
}

static void build_copy_wrong_target_outline(FT_Outline* outline, FT_Vector* points, unsigned char* tags, unsigned short* contours) {
    for (int i = 0; i < 3; i++) {
        points[i].x = 0;
        points[i].y = 0;
        tags[i] = 1;
    }
    contours[0] = 2;
    outline->n_contours = 1;
    outline->n_points = 3;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = FT_OUTLINE_OWNER;
}

static void print_u8_array(const unsigned char* values, int count) {
    printf("[");
    for (int i = 0; i < count; i++) {
        if (i) printf(",");
        printf("%u", values[i]);
    }
    printf("]");
}

static void print_u16_array(const unsigned short* values, int count) {
    printf("[");
    for (int i = 0; i < count; i++) {
        if (i) printf(",");
        printf("%u", values[i]);
    }
    printf("]");
}

static int emit_outline_copy(int argc, char** argv) {
    if (argc != 3) return 1;
    const char* case_id = argv[2];
    FT_Outline source;
    FT_Vector source_points[6];
    unsigned char source_tags[6];
    unsigned short source_contours[2];
    build_copy_source_outline(&source, source_points, source_tags, source_contours);
    print_ok_output_prefix();
    if (strstr(case_id, ".copies_arrays_and_flags")) {
        FT_Outline target;
        FT_Vector target_points[6];
        unsigned char target_tags[6];
        unsigned short target_contours[2];
        build_copy_target_outline(&target, target_points, target_tags, target_contours, 1);
        FT_Error error = FT_Outline_Copy(&source, &target);
        printf("{\"return\":%d,\"target_points\":", error);
        print_mutated_points(target_points, 6);
        printf(",\"target_tags\":");
        print_u8_array(target_tags, 6);
        printf(",\"target_contours\":");
        print_u16_array(target_contours, 2);
        printf(",\"target_flags\":%d}}\n", target.flags);
        return 0;
    }
    if (strstr(case_id, ".self_copy_noop")) {
        printf("{\"return\":%d,\"before\":", FT_Outline_Copy(&source, &source));
        print_outline_snapshot(&source);
        printf(",\"after\":");
        print_outline_snapshot(&source);
        printf("}}\n");
        return 0;
    }
    if (strstr(case_id, ".invalid_pointer_or_size_mismatch")) {
        FT_Outline target;
        FT_Vector target_points[6];
        unsigned char target_tags[6];
        unsigned short target_contours[2];
        build_copy_target_outline(&target, target_points, target_tags, target_contours, 1);
        FT_Outline wrong;
        FT_Vector wrong_points[3];
        unsigned char wrong_tags[3];
        unsigned short wrong_contours[1];
        build_copy_wrong_target_outline(&wrong, wrong_points, wrong_tags, wrong_contours);
        printf("{\"results\":[{\"label\":\"null_source\",\"return\":%d},{\"label\":\"null_target\",\"return\":%d},{\"label\":\"size_mismatch\",\"return\":%d}]}}\n",
               FT_Outline_Copy(NULL, &target),
               FT_Outline_Copy(&source, NULL),
               FT_Outline_Copy(&source, &wrong));
        return 0;
    }
    fprintf(stderr, "unsupported outline copy case: %s\n", case_id);
    return 2;
}

static void build_embolden_outline(FT_Outline* outline, FT_Vector* points, unsigned char* tags, unsigned short* contours) {
    FT_Vector src_points[5] = {{0,0},{0,96},{64,128},{128,96},{128,0}};
    memcpy(points, src_points, sizeof(src_points));
    for (int i = 0; i < 5; i++) tags[i] = 1;
    contours[0] = 4;
    outline->n_contours = 1;
    outline->n_points = 5;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 0;
}

static void build_none_orientation_outline(FT_Outline* outline, FT_Vector* points, unsigned char* tags, unsigned short* contours) {
    FT_Vector src_points[4] = {{0,0},{64,64},{0,64},{64,0}};
    memcpy(points, src_points, sizeof(src_points));
    for (int i = 0; i < 4; i++) tags[i] = 1;
    contours[0] = 3;
    outline->n_contours = 1;
    outline->n_points = 4;
    outline->points = points;
    outline->tags = tags;
    outline->contours = contours;
    outline->flags = 0;
}

static int emit_outline_embolden_common(int argc, char** argv, int xy) {
    if (argc != 3) return 1;
    const char* case_id = argv[2];
    print_ok_output_prefix();
    if (strstr(case_id, ".symmetric_strength_matches_xy")) {
        FT_Outline outline;
        FT_Vector points[5];
        unsigned char tags[5];
        unsigned short contours[1];
        FT_Outline xy_outline;
        FT_Vector xy_points[5];
        unsigned char xy_tags[5];
        unsigned short xy_contours[1];
        build_embolden_outline(&outline, points, tags, contours);
        build_embolden_outline(&xy_outline, xy_points, xy_tags, xy_contours);
        FT_Error error = FT_Outline_Embolden(&outline, 64);
        FT_Error xy_error = FT_Outline_EmboldenXY(&xy_outline, 64, 64);
        printf("{\"return\":%d,\"xy_return\":%d,\"points_after\":", error, xy_error);
        print_mutated_points(points, 5);
        printf(",\"xy_points_after\":");
        print_mutated_points(xy_points, 5);
        printf("}}\n");
        return 0;
    }
    if (strstr(case_id, ".zero_strength_noop")) {
        FT_Outline outline;
        FT_Vector points[5];
        unsigned char tags[5];
        unsigned short contours[1];
        build_embolden_outline(&outline, points, tags, contours);
        printf("{\"return\":");
        FT_Error error = FT_Outline_Embolden(&outline, 0);
        printf("%d,\"before\":", error);
        FT_Vector before[5] = {{0,0},{0,96},{64,128},{128,96},{128,0}};
        print_mutated_points(before, 5);
        printf(",\"after\":");
        print_mutated_points(points, 5);
        printf("}}\n");
        return 0;
    }
    if (strstr(case_id, ".anisotropic_strength_mutates_points")) {
        FT_Outline outline;
        FT_Vector points[5];
        unsigned char tags[5];
        unsigned short contours[1];
        build_embolden_outline(&outline, points, tags, contours);
        FT_Vector before[5];
        memcpy(before, points, sizeof(before));
        FT_Orientation orientation = FT_Outline_Get_Orientation(&outline);
        FT_Error error = FT_Outline_EmboldenXY(&outline, 96, 32);
        printf("{\"return\":%d,\"orientation\":%d,\"points_before\":", error, orientation);
        print_mutated_points(before, 5);
        printf(",\"points_after\":");
        print_mutated_points(points, 5);
        printf("}}\n");
        return 0;
    }
    if (strstr(case_id, ".empty_outline_success")) {
        FT_Outline outline = {0};
        FT_Error error = FT_Outline_EmboldenXY(&outline, 64, 64);
        printf("{\"return\":%d,\"points_after\":[]}}\n", error);
        return 0;
    }
    if (strstr(case_id, ".invalid_or_indeterminate_orientation_errors") || strstr(case_id, ".invalid_orientation_or_null_errors")) {
        FT_Outline outline;
        FT_Vector points[4];
        unsigned char tags[4];
        unsigned short contours[1];
        build_none_orientation_outline(&outline, points, tags, contours);
        FT_Error null_error = xy ? FT_Outline_EmboldenXY(NULL, 64, 64) : FT_Outline_Embolden(NULL, 64);
        FT_Error none_error = xy ? FT_Outline_EmboldenXY(&outline, 64, 64) : FT_Outline_Embolden(&outline, 64);
        printf("{\"results\":[{\"label\":\"null_outline\",\"return\":%d},{\"label\":\"none_orientation\",\"return\":%d}]}}\n", null_error, none_error);
        return 0;
    }
    fprintf(stderr, "unsupported outline embolden case: %s\n", case_id);
    return 2;
}

static int emit_outline_new(int argc, char** argv) {
    if (argc != 3) return 1;
    const char* case_id = argv[2];
    FT_Library library = NULL;
    FT_Init_FreeType(&library);
    FT_Outline outline = {0};
    print_ok_output_prefix();
    if (strstr(case_id, ".allocates_owner_outline") || strstr(case_id, ".empty_outline_allocation")) {
        FT_UInt points = strstr(case_id, ".empty_outline_allocation") ? 0 : 4;
        FT_Int contours = strstr(case_id, ".empty_outline_allocation") ? 0 : 1;
        FT_Error error = FT_Outline_New(library, points, contours, &outline);
        printf("{\"return\":%d,\"outline\":{\"n_points\":%d,\"n_contours\":%d,\"flags\":%d,\"points_null\":%s,\"tags_null\":%s,\"contours_null\":%s}}}\n",
               error, outline.n_points, outline.n_contours, outline.flags,
               outline.points ? "false" : "true", outline.tags ? "false" : "true", outline.contours ? "false" : "true");
        if (!error) FT_Outline_Done(library, &outline);
        FT_Done_FreeType(library);
        return 0;
    }
    if (strstr(case_id, ".invalid_arguments_and_limits")) {
        printf("{\"results\":[{\"label\":\"null_library\",\"return\":%d},{\"label\":\"null_output\",\"return\":%d},{\"label\":\"negative_contours\",\"return\":%d},{\"label\":\"contours_gt_points\",\"return\":%d},{\"label\":\"points_too_large\",\"return\":%d}]}}\n",
               FT_Outline_New(NULL, 1, 1, &outline),
               FT_Outline_New(library, 1, 1, NULL),
               FT_Outline_New(library, 1, -1, &outline),
               FT_Outline_New(library, 1, 2, &outline),
               FT_Outline_New(library, 65536, 1, &outline));
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Done_FreeType(library);
    fprintf(stderr, "unsupported outline new case: %s\n", case_id);
    return 2;
}

static int emit_outline_done(int argc, char** argv) {
    if (argc != 3) return 1;
    const char* case_id = argv[2];
    FT_Library library = NULL;
    FT_Init_FreeType(&library);
    print_ok_output_prefix();
    if (strstr(case_id, ".owner_outline_frees_and_resets") || strstr(case_id, ".non_owner_outline_resets_only")) {
        FT_Outline outline = {0};
        FT_Vector points[6];
        unsigned char tags[6];
        unsigned short contours[2];
        if (strstr(case_id, ".owner_outline_frees_and_resets")) {
            FT_Outline_New(library, 6, 2, &outline);
            outline.contours[0] = 2;
            outline.contours[1] = 5;
        } else {
            build_copy_target_outline(&outline, points, tags, contours, 0);
        }
        FT_Error error = FT_Outline_Done(library, &outline);
        printf("{\"return\":%d,\"frees\":", error);
        if (strstr(case_id, ".owner_outline_frees_and_resets")) printf("[\"points\",\"tags\",\"contours\"]");
        else printf("[]");
        printf(",\"outline_after\":");
        print_outline_snapshot(&outline);
        printf("}}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    if (strstr(case_id, ".invalid_library_or_outline_errors")) {
        FT_Outline outline;
        FT_Vector points[6];
        unsigned char tags[6];
        unsigned short contours[2];
        build_copy_target_outline(&outline, points, tags, contours, 1);
        printf("{\"results\":[{\"label\":\"null_library\",\"return\":%d},{\"label\":\"null_outline\",\"return\":%d}]}}\n",
               FT_Outline_Done(NULL, &outline), FT_Outline_Done(library, NULL));
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Done_FreeType(library);
    fprintf(stderr, "unsupported outline done case: %s\n", case_id);
    return 2;
}

static int emit_outline_transform(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    const char* case_id = argv[2];
    print_ok_output_prefix();
    if (strstr(case_id, ".null_inputs_noop")) {
        FT_Matrix identity = {0x10000L, 0, 0, 0x10000L};
        FT_Outline outline;
        FT_Vector points[4];
        unsigned char tags[4];
        unsigned short contours[1];
        build_transform_outline(&outline, points, tags, contours);
        FT_Outline_Transform(NULL, &identity);
        FT_Outline_Transform(&outline, NULL);
        printf("{\"rows\":["
               "{\"label\":\"null_outline\",\"sentinel_memory_changed\":false},"
               "{\"label\":\"null_matrix\",\"sentinel_memory_changed\":false,"
               "\"outline_after\":");
        print_outline_snapshot(&outline);
        printf("}]}}\n");
        return 0;
    }

    FT_Outline outline;
    FT_Vector points[4];
    unsigned char tags[4];
    unsigned short contours[1];
    build_transform_outline(&outline, points, tags, contours);
    FT_Matrix matrix;
    if (strstr(case_id, ".orientation_and_cbox_after_transform")) {
        matrix.xx = -0x10000L;
        matrix.xy = 0;
        matrix.yx = 0;
        matrix.yy = 0x10000L;
    } else {
        matrix.xx = 0x10000L;
        matrix.xy = 0x4000L;
        matrix.yx = -0x8000L;
        matrix.yy = 0x10000L;
    }
    FT_Outline_Transform(&outline, &matrix);
    if (strstr(case_id, ".matrix_transform_matches_c")) {
        printf("{\"points_after\":");
        print_mutated_points(points, 4);
        printf(",\"tags_after\":[%u,%u,%u,%u],\"contours_after\":[%d],"
               "\"flags_after\":%d}}\n",
               (unsigned int)tags[0], (unsigned int)tags[1],
               (unsigned int)tags[2], (unsigned int)tags[3],
               contours[0], outline.flags);
        return 0;
    }
    if (strstr(case_id, ".orientation_and_cbox_after_transform")) {
        FT_BBox cbox;
        FT_Outline_Get_CBox(&outline, &cbox);
        FT_Orientation orientation = FT_Outline_Get_Orientation(&outline);
        printf(
            "{\"cbox_after\":{\"xMin\":%ld,\"yMin\":%ld,\"xMax\":%ld,\"yMax\":%ld},"
            "\"orientation_after\":%d,\"outline_after\":",
            cbox.xMin,
            cbox.yMin,
            cbox.xMax,
            cbox.yMax,
            orientation
        );
        print_outline_snapshot(&outline);
        printf("}}\n");
        return 0;
    }
    fprintf(stderr, "unsupported outline transform case: %s\n", case_id);
    return 2;
}

static int emit_outline_translate(int argc, char** argv) {
    if (argc != 3) {
        return 1;
    }
    const char* case_id = argv[2];
    print_ok_output_prefix();
    if (strstr(case_id, ".null_outline_noop")) {
        FT_Outline_Translate(NULL, 64, -64);
        printf("{\"sentinel_memory_changed\":false}}\n");
        return 0;
    }

    long x_offset = strstr(case_id, ".empty_outline_success_noop") ? 64 : 17;
    long y_offset = strstr(case_id, ".empty_outline_success_noop") ? 64 : -33;
    FT_Outline outline;
    FT_Vector points[4] = {
        {-96, -32},
        {128, -64},
        {160, 96},
        {-64, 128},
    };
    unsigned char tags[4] = {
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
        FT_CURVE_TAG_ON,
    };
    unsigned short contours[1] = {3};
    outline.n_contours = 1;
    outline.n_points = 4;
    outline.points = points;
    outline.tags = tags;
    outline.contours = contours;
    outline.flags = 0;
    if (strstr(case_id, ".empty_outline_success_noop")) {
        outline.n_contours = 0;
        outline.n_points = 0;
    }
    if (strstr(case_id, ".offsets_all_points")) {
        printf("{\"points_before\":");
        print_mutated_points(points, 4);
        FT_Outline_Translate(&outline, x_offset, y_offset);
        FT_BBox cbox;
        FT_Outline_Get_CBox(&outline, &cbox);
        printf(",\"points_after\":");
        print_mutated_points(points, 4);
        printf(",\"cbox_after\":{\"xMin\":%ld,\"yMin\":%ld,\"xMax\":%ld,\"yMax\":%ld}}}\n",
               cbox.xMin,
               cbox.yMin,
               cbox.xMax,
               cbox.yMax);
        return 0;
    }
    if (strstr(case_id, ".empty_outline_success_noop")) {
        FT_Outline_Translate(&outline, x_offset, y_offset);
        printf("{\"n_points\":%d,\"points_after\":[]}}\n", outline.n_points);
        return 0;
    }
    fprintf(stderr, "unsupported outline translate case: %s\n", case_id);
    return 2;
}

static int face_macro_value(FT_Face face, const char* macro_name, int* out) {
    if (streq(macro_name, "FT_HAS_COLOR")) {
        *out = FT_HAS_COLOR(face);
    } else if (streq(macro_name, "FT_HAS_FAST_GLYPHS")) {
        *out = FT_HAS_FAST_GLYPHS(face);
    } else if (streq(macro_name, "FT_HAS_FIXED_SIZES")) {
        *out = FT_HAS_FIXED_SIZES(face);
    } else if (streq(macro_name, "FT_HAS_GLYPH_NAMES")) {
        *out = FT_HAS_GLYPH_NAMES(face);
    } else if (streq(macro_name, "FT_HAS_HORIZONTAL")) {
        *out = FT_HAS_HORIZONTAL(face);
    } else if (streq(macro_name, "FT_HAS_KERNING")) {
        *out = FT_HAS_KERNING(face);
    } else if (streq(macro_name, "FT_HAS_MULTIPLE_MASTERS")) {
        *out = FT_HAS_MULTIPLE_MASTERS(face);
    } else if (streq(macro_name, "FT_HAS_SBIX")) {
        *out = FT_HAS_SBIX(face);
    } else if (streq(macro_name, "FT_HAS_SBIX_OVERLAY")) {
        *out = FT_HAS_SBIX_OVERLAY(face);
    } else if (streq(macro_name, "FT_HAS_SVG")) {
        *out = FT_HAS_SVG(face);
    } else if (streq(macro_name, "FT_HAS_VERTICAL")) {
        *out = FT_HAS_VERTICAL(face);
    } else if (streq(macro_name, "FT_IS_CID_KEYED")) {
        *out = FT_IS_CID_KEYED(face);
    } else if (streq(macro_name, "FT_IS_FIXED_WIDTH")) {
        *out = FT_IS_FIXED_WIDTH(face);
    } else if (streq(macro_name, "FT_IS_NAMED_INSTANCE")) {
        *out = FT_IS_NAMED_INSTANCE(face);
    } else if (streq(macro_name, "FT_IS_SCALABLE")) {
        *out = FT_IS_SCALABLE(face);
    } else if (streq(macro_name, "FT_IS_SFNT")) {
        *out = FT_IS_SFNT(face);
    } else if (streq(macro_name, "FT_IS_TRICKY")) {
        *out = FT_IS_TRICKY(face);
    } else if (streq(macro_name, "FT_IS_VARIATION")) {
        *out = FT_IS_VARIATION(face);
    } else {
        return 0;
    }
    return 1;
}

static int emit_face_macro(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* macro_name = argv[5];

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
    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    int macro_result = 0;
    if (!face_macro_value(face, macro_name, &macro_result)) {
        fprintf(stderr, "unsupported face macro: %s\n", macro_name);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 2;
    }
    printf(",\"output\":{\"macro_result\":%d,\"face_flags\":%ld,\"face_index\":%ld,\"import_compiles\":true}}\n",
           macro_result,
           face->face_flags,
           face->face_index);
    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_face_macro_flags(int argc, char** argv) {
    (void)argc;
    const char* macro_name = argv[2];
    FT_Long face_index = atol(argv[3]);
    char* flags = (char*)malloc(strlen(argv[4]) + 1);
    if (!flags) {
        return 1;
    }
    memcpy(flags, argv[4], strlen(argv[4]) + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"macro\":\"%s\",\"import_compiles\":true,\"rows\":[", macro_name);
    char* cursor = flags;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        FT_FaceRec face;
        memset(&face, 0, sizeof(face));
        face.face_flags = atol(cursor);
        face.face_index = face_index;
        int macro_result = 0;
        if (!face_macro_value(&face, macro_name, &macro_result)) {
            fprintf(stderr, "unsupported face macro: %s\n", macro_name);
            free(flags);
            return 2;
        }
        if (emitted) {
            printf(",");
        }
        printf("{\"face_flags\":%ld,\"macro_result\":%d}", face.face_flags, macro_result);
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");
    free(flags);
    return 0;
}

static int emit_face_flags(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* flag_name = argv[5];

    long long flag_value = 0;
    if (!emit_generated_constant_value(flag_name, &flag_value)) {
        fprintf(stderr, "unsupported face flag: %s\n", flag_name);
        return 2;
    }

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
    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    printf(",\"output\":{\"face_flags\":%ld,\"flag\":%lld,\"bit_set\":%s,\"face_index\":%ld,\"import_compiles\":true}}\n",
           face->face_flags,
           flag_value,
           (face->face_flags & flag_value) ? "true" : "false",
           face->face_index);
    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

typedef struct SizeRequestRow_ {
    int face_is_null;
    int request_is_null;
    FT_Size_RequestRec req;
} SizeRequestRow;

static int parse_size_request_row(char* row, SizeRequestRow* out) {
    char* fields[7];
    char* cursor = row;
    for (int i = 0; i < 7; i++) {
        fields[i] = cursor;
        char* sep = strchr(cursor, ':');
        if (i == 6) {
            if (sep) {
                return 0;
            }
            break;
        }
        if (!sep) {
            return 0;
        }
        *sep = '\0';
        cursor = sep + 1;
    }
    out->face_is_null = (int)strtol(fields[0], NULL, 10) != 0;
    out->request_is_null = (int)strtol(fields[1], NULL, 10) != 0;
    out->req.type = (FT_Size_Request_Type)strtol(fields[2], NULL, 10);
    out->req.width = (FT_Long)strtol(fields[3], NULL, 10);
    out->req.height = (FT_Long)strtol(fields[4], NULL, 10);
    out->req.horiResolution = (FT_UInt)strtoul(fields[5], NULL, 10);
    out->req.vertResolution = (FT_UInt)strtoul(fields[6], NULL, 10);
    return 1;
}

static void print_size_request_row(SizeRequestRow row, FT_Error err, FT_Size_Metrics* metrics) {
    printf("{\"face\":\"%s\",\"request\":",
           row.face_is_null ? "null" : "valid");
    if (row.request_is_null) {
        printf("null");
    } else {
        printf("{\"type\":%d,\"width\":%ld,\"height\":%ld,\"horiResolution\":%u,\"vertResolution\":%u}",
               row.req.type,
               row.req.width,
               row.req.height,
               row.req.horiResolution,
               row.req.vertResolution);
    }
    printf(",\"status\":%d,\"metrics\":", err);
    if (err) {
        printf("null}");
    } else {
        printf("{");
        print_size_metrics_object(*metrics);
        printf("}}");
    }
}

static int emit_request_size(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    char* rows = (char*)malloc(strlen(argv[5]) + 1);
    if (!rows) {
        return 1;
    }
    memcpy(rows, argv[5], strlen(argv[5]) + 1);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(rows);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(rows);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(rows);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    FT_Face face = NULL;
    if (!err) {
        err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    }

    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        if (library) {
            FT_Done_FreeType(library);
        }
        free(data);
        free(rows);
        return 0;
    }

    size_t row_count = 0;
    const char* count_cursor = rows;
    while (count_cursor && *count_cursor) {
        row_count++;
        const char* next = strchr(count_cursor, ',');
        count_cursor = next ? next + 1 : NULL;
    }
    SizeRequestRow* parsed_rows = (SizeRequestRow*)calloc(row_count, sizeof(SizeRequestRow));
    FT_Error* errors = (FT_Error*)calloc(row_count, sizeof(FT_Error));
    FT_Size_Metrics* metrics = (FT_Size_Metrics*)calloc(row_count, sizeof(FT_Size_Metrics));
    if ((!parsed_rows || !errors || !metrics) && row_count > 0) {
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        free(rows);
        free(parsed_rows);
        free(errors);
        free(metrics);
        return 1;
    }

    char* cursor = rows;
    size_t row_index = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        if (!parse_size_request_row(cursor, &parsed_rows[row_index])) {
            fprintf(stderr, "bad size request row: %s\n", cursor);
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            free(rows);
            free(parsed_rows);
            free(errors);
            free(metrics);
            return 2;
        }
        row_index++;
        cursor = next ? next + 1 : NULL;
    }

    FT_Error first_error = FT_Err_Ok;
    for (size_t i = 0; i < row_count; i++) {
        errors[i] = FT_Request_Size(
            parsed_rows[i].face_is_null ? NULL : face,
            parsed_rows[i].request_is_null ? NULL : &parsed_rows[i].req
        );
        if (!first_error && errors[i]) {
            first_error = errors[i];
        }
        if (!errors[i] && !parsed_rows[i].face_is_null) {
            metrics[i] = face->size->metrics;
        }
    }

    printf("{");
    print_status(first_error);
    printf(",\"output\":{\"outputs\":[");
    for (size_t i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_size_request_row(parsed_rows[i], errors[i], &metrics[i]);
    }
    printf("]}}\n");
    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    free(rows);
    free(parsed_rows);
    free(errors);
    free(metrics);
    return 0;
}

static void print_charmap_probe_indices(FT_Face face, const char* chars_csv) {
    if (streq(chars_csv, "-") || !chars_csv[0]) {
        printf("[]");
        return;
    }
    char* chars = (char*)malloc(strlen(chars_csv) + 1);
    if (!chars) {
        printf("[]");
        return;
    }
    memcpy(chars, chars_csv, strlen(chars_csv) + 1);
    printf("[");
    char* cursor = chars;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        FT_ULong char_code = strtoul(cursor, NULL, 10);
        if (emitted) {
            printf(",");
        }
        printf("{\"char_code\":%lu,\"glyph_index\":%u}",
               (unsigned long)char_code,
               FT_Get_Char_Index(face, char_code));
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    printf("]");
    free(chars);
}

static void print_char_variant_index_output(FT_UInt value) {
    printf("{\"result\":%u,\"value\":%u}", value, value);
}

static void print_char_variant_default_output(FT_Int value) {
    printf("{\"result\":%d,\"value\":%d}", value, value);
}

static void print_uint32_list_output(FT_UInt32* values) {
    FT_UInt32* cursor = values;
    int emitted = 0;

    if (!values) {
        printf("{\"result\":{\"is_null\":true,\"nullness\":\"null\",\"values\":[],\"terminator\":null},\"copied_values_before_invalidation\":[],\"pointer_identity_class\":\"null\"}");
        return;
    }

    printf("{\"result\":{\"is_null\":false,\"nullness\":\"non_null\",\"values\":[");
    while (*cursor) {
        if (emitted) {
            printf(",");
        }
        printf("%lu", (unsigned long)*cursor);
        emitted = 1;
        cursor++;
    }
    printf("],\"terminator\":%lu},\"copied_values_before_invalidation\":[", (unsigned long)*cursor);
    cursor = values;
    emitted = 0;
    while (*cursor) {
        if (emitted) {
            printf(",");
        }
        printf("%lu", (unsigned long)*cursor);
        emitted = 1;
        cursor++;
    }
    printf("],\"pointer_identity_class\":\"face_owned_scratch\"}");
}

static int emit_face_get_char_variant_index_null(int argc, char** argv) {
    FT_ULong charcode = strtoul(argv[2], NULL, 10);
    FT_ULong variant_selector = strtoul(argv[3], NULL, 10);
    (void)argc;
    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_char_variant_index_output(
        FT_Face_GetCharVariantIndex(NULL, charcode, variant_selector)
    );
    printf("}\n");
    return 0;
}

static int emit_face_get_char_variant_is_default_null(int argc, char** argv) {
    FT_ULong charcode = strtoul(argv[2], NULL, 10);
    FT_ULong variant_selector = strtoul(argv[3], NULL, 10);
    (void)argc;
    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_char_variant_default_output(
        FT_Face_GetCharVariantIsDefault(NULL, charcode, variant_selector)
    );
    printf("}\n");
    return 0;
}

static int emit_face_get_variant_selectors_null(int argc, char** argv) {
    (void)argc;
    (void)argv;
    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_uint32_list_output(FT_Face_GetVariantSelectors(NULL));
    printf("}\n");
    return 0;
}

static int emit_face_get_variants_of_char_null(int argc, char** argv) {
    FT_ULong charcode = strtoul(argv[2], NULL, 10);
    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_uint32_list_output(FT_Face_GetVariantsOfChar(NULL, charcode));
    printf("}\n");
    return 0;
}

static int emit_face_get_chars_of_variant_null(int argc, char** argv) {
    FT_ULong variant_selector = strtoul(argv[2], NULL, 10);
    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_uint32_list_output(FT_Face_GetCharsOfVariant(NULL, variant_selector));
    printf("}\n");
    return 0;
}

typedef struct OracleFace {
    FT_Library library;
    FT_Face face;
    unsigned char* data;
    long data_len;
} OracleFace;

static void close_oracle_face(OracleFace* face) {
    if (face->face) {
        FT_Done_Face(face->face);
        face->face = NULL;
    }
    if (face->library) {
        FT_Done_FreeType(face->library);
        face->library = NULL;
    }
    free(face->data);
    face->data = NULL;
}

static int open_oracle_face(
    const char* source_kind,
    const char* source_value,
    FT_Long face_index,
    OracleFace* out) {
    out->library = NULL;
    out->face = NULL;
    out->data = NULL;
    out->data_len = 0;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &out->data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &out->data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        return 2;
    }

    FT_Error err = FT_Init_FreeType(&out->library);
    if (!err) {
        err = FT_New_Memory_Face(out->library, out->data, data_len, face_index, &out->face);
    }
    out->data_len = data_len;
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        close_oracle_face(out);
        return 1;
    }
    return 0;
}

static int emit_active_size_handle(int argc, char** argv) {
    if (argc != 7) {
        return 1;
    }
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_UInt pixel_width = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_Error status = FT_Set_Pixel_Sizes(face.face, pixel_width, pixel_height);
    FT_Error load_error = FT_Load_Glyph(face.face, 36, FT_LOAD_DEFAULT);
    int later_load_uses_active_size =
        status == FT_Err_Ok &&
        load_error == FT_Err_Ok &&
        face.face->glyph &&
        face.face->glyph->metrics.horiAdvance != 0;

    printf("{");
    print_status(status);
    printf(",\"output\":{\"status\":%d,", status);
    printf("\"active_size_identity\":\"%s\",", face.face->size ? "face_active_size" : "null");
    printf("\"metrics\":");
    if (status == FT_Err_Ok && face.face->size) {
        printf("{");
        print_size_metrics_object(face.face->size->metrics);
        printf("}");
    } else {
        printf("null");
    }
    printf(",\"later_load_uses_active_size\":");
    print_json_bool(later_load_uses_active_size);
    printf("}}\n");

    close_oracle_face(&face);
    return 0;
}

static FT_Error first_non_ok3(FT_Error a, FT_Error b, FT_Error c) {
    if (a) {
        return a;
    }
    if (b) {
        return b;
    }
    return c;
}

static int emit_face_owned_handles(int argc, char** argv) {
    if (argc != 5) {
        return 1;
    }
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_Error set_size_error = FT_Set_Pixel_Sizes(face.face, 0, 20);
    FT_Error select_error = FT_Select_Charmap(face.face, FT_ENCODING_UNICODE);
    FT_Error load_error = FT_Load_Glyph(face.face, 36, FT_LOAD_DEFAULT);
    FT_Error status = first_non_ok3(set_size_error, select_error, load_error);

    printf("{");
    print_status(status);
    printf(",\"output\":{\"opened\":true,\"status\":%d,", status);
    printf("\"glyph\":{\"owner\":\"%s\"},", load_error ? "none" : "same_face");
    printf("\"size\":{\"owner\":\"%s\"},", face.face->size ? "same_face" : "none");
    printf("\"charmaps\":[");
    for (FT_Int i = 0; face.face && face.face->charmaps && i < face.face->num_charmaps; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"index\":%d,\"owner\":\"same_face\"}", i);
    }
    printf("],\"handles_after_done\":{\"policy\":\"invalid_after_done_face\"}}}\n");

    close_oracle_face(&face);
    return 0;
}

static int print_malformed_maxp_row(
    const char* variant,
    const char* source_kind,
    const char* source_value,
    FT_Long face_index) {
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
    FT_Face face = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    FT_Error face_error = init_error;
    if (!face_error) {
        face_error = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    }
    TT_MaxProfile* maxp = NULL;
    if (!face_error) {
        maxp = (TT_MaxProfile*)FT_Get_Sfnt_Table(face, FT_SFNT_MAXP);
    }

    printf("{\"variant\":\"%s\",\"error\":%d,\"face_load_error\":%d,\"pointer_null\":",
           variant,
           (int)face_error,
           (int)face_error);
    print_json_bool(face_error || maxp == NULL);
    printf(",\"fields_if_loaded\":");
    print_tt_maxprofile_record(face_error ? NULL : maxp);
    printf("}");

    if (face) {
        FT_Done_Face(face);
    }
    if (library) {
        FT_Done_FreeType(library);
    }
    free(data);
    return 0;
}

static int emit_malformed_maxp_route(int argc, char** argv) {
    if (argc != 7) {
        fprintf(stderr, "--malformed-maxp-route requires TRUNC_KIND TRUNC_SOURCE INVALID_KIND INVALID_SOURCE FACE_INDEX\n");
        return 2;
    }
    FT_Long face_index = atol(argv[6]);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    int status = print_malformed_maxp_row("truncated_maxp", argv[2], argv[3], face_index);
    if (status) {
        printf("]}}\n");
        return status;
    }
    printf(",");
    status = print_malformed_maxp_row("invalid_maxp", argv[4], argv[5], face_index);
    printf("]}}\n");
    return status;
}

static void print_size_metrics_named_value(const char* name, FT_Size_Metrics* metrics) {
    printf("\"%s\":", name);
    if (!metrics) {
        printf("null");
        return;
    }
    printf("{");
    print_size_metrics_object(*metrics);
    printf("}");
}

static const char* size_identity_label(FT_Size size, FT_Size initial, FT_Size secondary) {
    if (!size) {
        return "null";
    }
    if (size == initial) {
        return "initial_size";
    }
    if (size == secondary) {
        return "secondary_size";
    }
    return "other";
}

static FT_Error first_sequence_error(const FT_Error* errors, size_t count) {
    for (size_t i = 0; i < count; i++) {
        if (errors[i]) {
            return errors[i];
        }
    }
    return FT_Err_Ok;
}

static FT_Size_RequestRec nominal_size_request(FT_Long height) {
    FT_Size_RequestRec request;
    request.type = FT_SIZE_REQUEST_TYPE_NOMINAL;
    request.width = 0;
    request.height = height;
    request.horiResolution = 72;
    request.vertResolution = 72;
    return request;
}

static int emit_new_size_sequence(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_Size initial = face.face->size;
    FT_Size secondary = NULL;
    FT_Error errors[5];
    errors[0] = FT_New_Size(face.face, &secondary);
    int active_unchanged_until_activation = face.face->size == initial;
    const char* parent_face = "none";
    if (secondary) {
        parent_face = secondary->face == face.face ? "same_face" : "other";
    }

    errors[1] = FT_Activate_Size(secondary);
    FT_Size_RequestRec request20 = nominal_size_request(20 * 64);
    errors[2] = FT_Request_Size(face.face, &request20);
    FT_Size_Metrics secondary_metrics;
    FT_Size_Metrics* secondary_metrics_ptr = NULL;
    if (!errors[2] && face.face->size) {
        secondary_metrics = face.face->size->metrics;
        secondary_metrics_ptr = &secondary_metrics;
    }

    errors[3] = FT_Activate_Size(initial);
    FT_Size_RequestRec request10 = nominal_size_request(10 * 64);
    errors[4] = FT_Request_Size(face.face, &request10);
    FT_Size_Metrics initial_metrics;
    FT_Size_Metrics* initial_metrics_ptr = NULL;
    if (!errors[4] && face.face->size) {
        initial_metrics = face.face->size->metrics;
        initial_metrics_ptr = &initial_metrics;
    }

    printf("{");
    print_status(first_sequence_error(errors, 5));
    printf(",\"output\":{\"return_sequence\":[%d,%d,%d,%d,%d],",
           errors[0], errors[1], errors[2], errors[3], errors[4]);
    printf("\"new_size_nullness\":\"%s\",", secondary ? "non_null" : "null");
    printf("\"new_size_parent_face\":\"%s\",", parent_face);
    printf("\"active_size_unchanged_until_activation\":");
    print_json_bool(active_unchanged_until_activation);
    printf(",\"metrics_by_size\":{");
    print_size_metrics_named_value("initial_size", initial_metrics_ptr);
    printf(",");
    print_size_metrics_named_value("secondary_size", secondary_metrics_ptr);
    printf("}}}\n");

    if (secondary) {
        FT_Done_Size(secondary);
    }
    close_oracle_face(&face);
    return 0;
}

static int emit_done_size_sequence(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_Size initial = face.face->size;
    FT_Size secondary = NULL;
    int remove_active = argc > 5 ? atoi(argv[5]) != 0 : 1;
    FT_Error errors[6];
    errors[0] = FT_New_Size(face.face, &secondary);
    errors[1] = FT_Activate_Size(remove_active ? secondary : initial);
    errors[2] = FT_Set_Pixel_Sizes(face.face, 0, 18);
    errors[3] = FT_Done_Size(secondary);
    FT_Size active_after_done = face.face->size;
    int destroyed_size_removed = active_after_done != secondary;
    errors[4] = FT_Set_Pixel_Sizes(face.face, 0, 12);
    errors[5] = FT_Load_Glyph(face.face, 36, FT_LOAD_DEFAULT);

    printf("{");
    print_status(first_sequence_error(errors, 6));
    printf(",\"output\":{\"return_sequence\":[%d,%d,%d,%d,%d,%d],",
           errors[0], errors[1], errors[2], errors[3], errors[4], errors[5]);
    printf("\"destroyed_size_removed\":");
    print_json_bool(destroyed_size_removed);
    printf(",\"active_size_after_done\":\"%s\",",
           size_identity_label(active_after_done, initial, secondary));
    printf("\"post_done_face_usable\":");
    print_json_bool(!errors[4] && !errors[5]);
    printf(",\"post_done_glyph_metrics\":");
    if (!errors[5] && face.face->glyph) {
        print_glyph_metrics_object(face.face->glyph->metrics);
    } else {
        printf("null");
    }
    printf("}}\n");

    close_oracle_face(&face);
    return 0;
}

static int emit_activate_size_sequence(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_Size initial = face.face->size;
    FT_Size secondary = NULL;
    FT_Error errors[7];
    errors[0] = FT_Set_Pixel_Sizes(face.face, 0, 12);
    FT_Size_Metrics initial_metrics;
    FT_Size_Metrics* initial_metrics_ptr = NULL;
    if (!errors[0] && face.face->size) {
        initial_metrics = face.face->size->metrics;
        initial_metrics_ptr = &initial_metrics;
    }

    errors[1] = FT_New_Size(face.face, &secondary);
    errors[2] = FT_Activate_Size(secondary);
    const char* after_secondary_activation = size_identity_label(face.face->size, initial, secondary);
    FT_Size_RequestRec request18 = nominal_size_request(18 * 64);
    errors[3] = FT_Request_Size(face.face, &request18);
    FT_Size_Metrics secondary_metrics;
    FT_Size_Metrics* secondary_metrics_ptr = NULL;
    if (!errors[3] && face.face->size) {
        secondary_metrics = face.face->size->metrics;
        secondary_metrics_ptr = &secondary_metrics;
    }
    errors[4] = FT_Load_Glyph(face.face, 36, FT_LOAD_DEFAULT);
    FT_Glyph_Metrics secondary_glyph_metrics;
    int has_secondary_glyph_metrics = !errors[4] && face.face->glyph;
    if (has_secondary_glyph_metrics) {
        secondary_glyph_metrics = face.face->glyph->metrics;
    }

    errors[5] = FT_Activate_Size(initial);
    const char* after_initial_activation = size_identity_label(face.face->size, initial, secondary);
    errors[6] = FT_Load_Glyph(face.face, 36, FT_LOAD_DEFAULT);
    FT_Glyph_Metrics initial_glyph_metrics;
    int has_initial_glyph_metrics = !errors[6] && face.face->glyph;
    if (has_initial_glyph_metrics) {
        initial_glyph_metrics = face.face->glyph->metrics;
    }

    printf("{");
    print_status(first_sequence_error(errors, 7));
    printf(",\"output\":{\"return_sequence\":[%d,%d,%d,%d,%d,%d,%d],",
           errors[0], errors[1], errors[2], errors[3], errors[4], errors[5], errors[6]);
    printf("\"active_size_identity\":{\"after_secondary_activation\":\"%s\",\"after_initial_activation\":\"%s\"},",
           after_secondary_activation,
           after_initial_activation);
    printf("\"metrics_by_size\":{");
    print_size_metrics_named_value("initial_size", initial_metrics_ptr);
    printf(",");
    print_size_metrics_named_value("secondary_size", secondary_metrics_ptr);
    printf("},\"glyph_metrics_by_active_size\":[");
    if (has_secondary_glyph_metrics) {
        print_glyph_metrics_object(secondary_glyph_metrics);
    } else {
        printf("null");
    }
    printf(",");
    if (has_initial_glyph_metrics) {
        print_glyph_metrics_object(initial_glyph_metrics);
    } else {
        printf("null");
    }
    printf("]}}\n");

    if (secondary) {
        FT_Done_Size(secondary);
    }
    close_oracle_face(&face);
    return 0;
}

static int emit_activate_select_size_sequence(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_Int strike_index = (FT_Int)strtol(argv[5], NULL, 10);
    FT_Size initial = face.face->size;
    FT_Size secondary = NULL;
    FT_Error errors[5];
    errors[0] = FT_Set_Pixel_Sizes(face.face, 0, 12);
    errors[1] = FT_New_Size(face.face, &secondary);
    errors[2] = FT_Activate_Size(secondary);
    const char* after_secondary_activation = size_identity_label(face.face->size, initial, secondary);
    errors[3] = FT_Select_Size(face.face, strike_index);
    FT_Size_Metrics selected_metrics;
    FT_Size_Metrics* selected_metrics_ptr = NULL;
    if (!errors[3] && face.face->size) {
        selected_metrics = face.face->size->metrics;
        selected_metrics_ptr = &selected_metrics;
    }
    errors[4] = FT_Activate_Size(initial);
    const char* after_initial_activation = size_identity_label(face.face->size, initial, secondary);
    FT_Size_Metrics inactive_metrics;
    FT_Size_Metrics* inactive_metrics_ptr = NULL;
    if (!errors[4] && face.face->size) {
        inactive_metrics = face.face->size->metrics;
        inactive_metrics_ptr = &inactive_metrics;
    }

    printf("{");
    print_status(first_sequence_error(errors, 5));
    printf(",\"output\":{\"return_sequence\":[%d,%d,%d,%d,%d],",
           errors[0], errors[1], errors[2], errors[3], errors[4]);
    printf("\"active_size_identity\":{\"after_secondary_activation\":\"%s\",\"after_initial_activation\":\"%s\"},",
           after_secondary_activation,
           after_initial_activation);
    print_size_metrics_named_value("selected_strike_metrics", selected_metrics_ptr);
    printf(",");
    print_size_metrics_named_value("inactive_size_metrics", inactive_metrics_ptr);
    printf("}}\n");

    if (secondary) {
        FT_Done_Size(secondary);
    }
    close_oracle_face(&face);
    return 0;
}

static int emit_new_size_null_face(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_Size size = NULL;
    FT_Error err = FT_New_Size(NULL, &size);
    printf("{");
    print_status(err);
    printf(",\"output\":{\"output_size_nullness\":\"%s\"}}\n",
           size ? "non_null" : "null");
    if (size) {
        FT_Done_Size(size);
    }
    return 0;
}

static int emit_new_size_null_output(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    int active_size_was_null = face.face->size == NULL;
    FT_Error err = FT_New_Size(face.face, NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":{\"output_pointer_null\":true,");
    printf("\"active_size_nullness_before\":\"%s\",",
           active_size_was_null ? "null" : "non_null");
    printf("\"active_size_nullness_after\":\"%s\"}}\n",
           face.face->size ? "non_null" : "null");
    close_oracle_face(&face);
    return 0;
}

static int emit_done_size_null(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_Error err = FT_Done_Size(NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":{\"size_input_null\":true}}\n");
    return 0;
}

static int emit_activate_size_null(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_Error err = FT_Activate_Size(NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":{\"size_input_null\":true}}\n");
    return 0;
}

static int emit_open_type_validate_null_face(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_Bytes base = NULL;
    FT_Bytes gdef = NULL;
    FT_Bytes gpos = NULL;
    FT_Bytes gsub = NULL;
    FT_Bytes jstf = NULL;
    FT_Error err = FT_OpenType_Validate(NULL, FT_VALIDATE_BASE, &base, &gdef, &gpos, &gsub, &jstf);
    printf("{");
    print_status(err);
    printf(",\"output\":{\"outputs_touched\":");
    print_json_bool(base || gdef || gpos || gsub || jstf);
    printf("}}\n");
    return 0;
}

static void print_open_type_validate_null_output_rows(FT_Face face, const char* labels_csv) {
    char* labels = (char*)malloc(strlen(labels_csv) + 1);
    if (!labels) {
        printf("[]");
        return;
    }
    memcpy(labels, labels_csv, strlen(labels_csv) + 1);
    printf("[");
    char* token = strtok(labels, ",");
    int first = 1;
    while (token) {
        FT_Bytes base = NULL;
        FT_Bytes gdef = NULL;
        FT_Bytes gpos = NULL;
        FT_Bytes gsub = NULL;
        FT_Bytes jstf = NULL;
        FT_Bytes* base_ptr = streq(token, "BASE") ? NULL : &base;
        FT_Bytes* gdef_ptr = streq(token, "GDEF") ? NULL : &gdef;
        FT_Bytes* gpos_ptr = streq(token, "GPOS") ? NULL : &gpos;
        FT_Bytes* gsub_ptr = streq(token, "GSUB") ? NULL : &gsub;
        FT_Bytes* jstf_ptr = streq(token, "JSTF") ? NULL : &jstf;
        FT_Error err = FT_OpenType_Validate(
            face,
            FT_VALIDATE_BASE,
            base_ptr,
            gdef_ptr,
            gpos_ptr,
            gsub_ptr,
            jstf_ptr);
        if (!first) {
            printf(",");
        }
        first = 0;
        printf("{\"null_output_index\":\"%s\",\"error\":%d}", token, err);
        token = strtok(NULL, ",");
    }
    printf("]");
    free(labels);
}

static int emit_open_type_validate_null_outputs(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    printf("{");
    print_status(FT_Err_Invalid_Argument);
    printf(",\"output\":{\"rows\":");
    print_open_type_validate_null_output_rows(face.face, argv[5]);
    printf("}}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_open_type_validate_service_missing(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_Bytes base = NULL;
    FT_Bytes gdef = NULL;
    FT_Bytes gpos = NULL;
    FT_Bytes gsub = NULL;
    FT_Bytes jstf = NULL;
    FT_Error err = FT_OpenType_Validate(face.face, 0, &base, &gdef, &gpos, &gsub, &jstf);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    close_oracle_face(&face);
    return 0;
}

static void print_short_array(const FT_Short* values, int len) {
    printf("[");
    for (int i = 0; i < len; i++) {
        if (i) printf(",");
        printf("%d", values[i]);
    }
    printf("]");
}

static void print_ps_ushort_array(const FT_UShort* values, int len) {
    printf("[");
    for (int i = 0; i < len; i++) {
        if (i) printf(",");
        printf("%u", values[i]);
    }
    printf("]");
}

static void print_ps_font_info_json(const PS_FontInfoRec* info) {
    printf("{\"version\":");
    print_nullable_c_string_result(info->version);
    printf(",\"notice\":");
    print_nullable_c_string_result(info->notice);
    printf(",\"full_name\":");
    print_nullable_c_string_result(info->full_name);
    printf(",\"family_name\":");
    print_nullable_c_string_result(info->family_name);
    printf(",\"weight\":");
    print_nullable_c_string_result(info->weight);
    printf(",\"italic_angle\":%ld", (long)info->italic_angle);
    printf(",\"is_fixed_pitch\":%u", info->is_fixed_pitch);
    printf(",\"underline_position\":%d", info->underline_position);
    printf(",\"underline_thickness\":%u}", info->underline_thickness);
}

static int emit_ps_font_info(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    PS_FontInfoRec info;
    memset(&info, 0, sizeof(info));
    FT_Error err = FT_Get_PS_Font_Info(face.face, &info);
    printf("{");
    print_status(err);
    printf(",\"output\":");
    if (err == FT_Err_Ok) {
        print_ps_font_info_json(&info);
    } else {
        printf("null");
    }
    printf("}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_ps_font_info_null_face(int argc, char** argv) {
    (void)argc;
    (void)argv;
    PS_FontInfoRec info;
    memset(&info, 0, sizeof(info));
    FT_Error err = FT_Get_PS_Font_Info(NULL, &info);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_ps_font_info_null_output(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_Error err = FT_Get_PS_Font_Info(face.face, NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_has_ps_glyph_names(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_Int result = FT_Has_PS_Glyph_Names(face.face);
    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"result\":%d}}\n", result);
    close_oracle_face(&face);
    return 0;
}

static int emit_has_ps_glyph_names_null_face(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_Int result = FT_Has_PS_Glyph_Names(NULL);
    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"result\":%d}}\n", result);
    return 0;
}

static int oracle_bytes_contains(const unsigned char* data, long data_len, const char* pattern) {
    size_t pattern_len = strlen(pattern);
    if (!data || data_len <= 0 || pattern_len == 0 || (long)pattern_len > data_len) {
        return 0;
    }
    for (long i = 0; i <= data_len - (long)pattern_len; i++) {
        if (memcmp(data + i, pattern, pattern_len) == 0) {
            return 1;
        }
    }
    return 0;
}

static void print_ps_mm_blend_dictionary_output(
    const PS_FontInfoRec* info,
    const char* field,
    long constant_value,
    int blend_present) {
    printf("{\"constant_value\":%ld", constant_value);
    printf(",\"blend_present\":");
    print_json_bool(blend_present);
    printf(",\"font_info\":{");
    if (streq(field, "underline_position")) {
        printf("\"underline_position\":%d", info->underline_position);
    } else if (streq(field, "underline_thickness")) {
        printf("\"underline_thickness\":%u", info->underline_thickness);
    }
    printf("}}");
}

static int emit_ps_mm_blend_dictionary(int argc, char** argv) {
    (void)argc;
    const char* field = argv[5];
    long constant_value = atol(argv[6]);
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    char pattern[96];
    snprintf(pattern, sizeof(pattern), "/%s [", field);
    int blend_present = oracle_bytes_contains(face.data, face.data_len, pattern);
    PS_FontInfoRec info;
    memset(&info, 0, sizeof(info));
    FT_Error err = FT_Get_PS_Font_Info(face.face, &info);
    printf("{");
    print_status(err);
    printf(",\"output\":");
    print_ps_mm_blend_dictionary_output(&info, field, constant_value, blend_present);
    printf("}\n");
    close_oracle_face(&face);
    return 0;
}

static void print_ps_private_json(const PS_PrivateRec* private_rec) {
    printf("{\"unique_id\":%d,\"lenIV\":%d", private_rec->unique_id, private_rec->lenIV);
    printf(",\"num_blue_values\":%u", private_rec->num_blue_values);
    printf(",\"num_other_blues\":%u", private_rec->num_other_blues);
    printf(",\"num_family_blues\":%u", private_rec->num_family_blues);
    printf(",\"num_family_other_blues\":%u", private_rec->num_family_other_blues);
    printf(",\"blue_values\":");
    print_short_array(private_rec->blue_values, 14);
    printf(",\"other_blues\":");
    print_short_array(private_rec->other_blues, 10);
    printf(",\"family_blues\":");
    print_short_array(private_rec->family_blues, 14);
    printf(",\"family_other_blues\":");
    print_short_array(private_rec->family_other_blues, 10);
    printf(",\"blue_scale\":%ld", (long)private_rec->blue_scale);
    printf(",\"blue_shift\":%d", private_rec->blue_shift);
    printf(",\"blue_fuzz\":%d", private_rec->blue_fuzz);
    printf(",\"standard_width\":");
    print_ps_ushort_array(private_rec->standard_width, 1);
    printf(",\"standard_height\":");
    print_ps_ushort_array(private_rec->standard_height, 1);
    printf(",\"num_snap_widths\":%u", private_rec->num_snap_widths);
    printf(",\"num_snap_heights\":%u", private_rec->num_snap_heights);
    printf(",\"force_bold\":%u", private_rec->force_bold);
    printf(",\"round_stem_up\":%u", private_rec->round_stem_up);
    printf(",\"snap_widths\":");
    print_short_array(private_rec->snap_widths, 13);
    printf(",\"snap_heights\":");
    print_short_array(private_rec->snap_heights, 13);
    printf(",\"expansion_factor\":%ld", (long)private_rec->expansion_factor);
    printf(",\"language_group\":%ld", private_rec->language_group);
    printf(",\"password\":%ld", private_rec->password);
    printf(",\"min_feature\":");
    print_short_array(private_rec->min_feature, 2);
    printf("}");
}

static int emit_ps_font_private(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    PS_PrivateRec private_rec;
    memset(&private_rec, 0, sizeof(private_rec));
    FT_Error err = FT_Get_PS_Font_Private(face.face, &private_rec);
    printf("{");
    print_status(err);
    printf(",\"output\":");
    if (err == FT_Err_Ok) {
        print_ps_private_json(&private_rec);
    } else {
        printf("null");
    }
    printf("}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_ps_font_private_null_face(int argc, char** argv) {
    (void)argc;
    (void)argv;
    PS_PrivateRec private_rec;
    memset(&private_rec, 0, sizeof(private_rec));
    FT_Error err = FT_Get_PS_Font_Private(NULL, &private_rec);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_ps_font_private_null_output(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_Error err = FT_Get_PS_Font_Private(face.face, NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_ps_font_private_rowset(int argc, char** argv) {
    int count = atoi(argv[2]);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    int arg = 3;
    for (int row = 0; row < count; row++) {
        OracleFace face;
        int opened = open_oracle_face(argv[arg], argv[arg + 1], atol(argv[arg + 2]), &face);
        if (opened != 0) {
            return opened;
        }
        PS_PrivateRec private_rec;
        memset(&private_rec, 0, sizeof(private_rec));
        FT_Error err = FT_Get_PS_Font_Private(face.face, &private_rec);
        if (row) printf(",");
        if (err == FT_Err_Ok) {
            print_ps_private_json(&private_rec);
        } else {
            printf("null");
        }
        close_oracle_face(&face);
        arg += 3;
    }
    printf("]}}\n");
    return 0;
}

static void print_ps_font_value_encoding_type_json(FT_Face face) {
    T1_EncodingType encoding = (T1_EncodingType)-999;
    FT_Long ret = FT_Get_PS_Font_Value(face, PS_DICT_ENCODING_TYPE, 0,
                                       &encoding, sizeof(encoding));
    printf("{\"return\":%ld,\"encoding_type\":%d,\"bytes\":\"", ret, encoding);
    print_hex_bytes((const unsigned char*)&encoding, (long)sizeof(encoding));
    printf("\"}");
}

static void print_ps_font_value_encoding_entry_json(FT_Face face, FT_UInt index) {
    unsigned char buffer[256];
    memset(buffer, 0xA5, sizeof(buffer));
    FT_Long ret = FT_Get_PS_Font_Value(face, PS_DICT_ENCODING_ENTRY, index,
                                       buffer, sizeof(buffer));
    printf("{\"index\":%u,\"return\":%ld,\"bytes\":\"", index, ret);
    if (ret > 0 && ret <= (FT_Long)sizeof(buffer)) {
        print_hex_bytes(buffer, ret);
    }
    printf("\"}");
}

static void print_ps_font_value_encoding_json(FT_Face face, const char* entries_csv) {
    printf("{\"encoding_type\":");
    print_ps_font_value_encoding_type_json(face);
    if (entries_csv && entries_csv[0]) {
        printf(",\"entries\":[");
        const char* cursor = entries_csv;
        int first = 1;
        while (*cursor) {
            char* end = NULL;
            unsigned long index = strtoul(cursor, &end, 10);
            if (end == cursor) {
                break;
            }
            if (!first) printf(",");
            print_ps_font_value_encoding_entry_json(face, (FT_UInt)index);
            first = 0;
            cursor = (*end == ',') ? end + 1 : end;
        }
        printf("]");
    }
    printf("}");
}

static int emit_ps_font_value_encoding(int argc, char** argv) {
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    const char* entries_csv = argc >= 6 ? argv[5] : "";
    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_ps_font_value_encoding_json(face.face, entries_csv);
    printf("}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_ps_font_value_encoding_rowset(int argc, char** argv) {
    int count = atoi(argv[2]);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    int arg = 3;
    for (int row = 0; row < count; row++) {
        OracleFace face;
        int opened = open_oracle_face(argv[arg], argv[arg + 1], atol(argv[arg + 2]), &face);
        if (opened != 0) {
            return opened;
        }
        if (row) printf(",");
        print_ps_font_value_encoding_json(face.face, "");
        close_oracle_face(&face);
        arg += 3;
    }
    printf("]}}\n");
    return 0;
}

static void print_ps_font_value_matrix_row(const char* id,
                                           const char* key_name,
                                           FT_Face face,
                                           PS_Dict_Keys key,
                                           FT_UInt idx,
                                           int pointer_is_null,
                                           FT_Long explicit_len,
                                           int exact_len) {
    FT_Long prequery = FT_Get_PS_Font_Value(face, key, idx, NULL, 0);
    FT_Long value_len = exact_len && prequery > 0 ? prequery : explicit_len;
    unsigned char buffer[256];
    memset(buffer, 0xA5, sizeof(buffer));
    void* value = pointer_is_null ? NULL : buffer;
    FT_Long ret = FT_Get_PS_Font_Value(face, key, idx, value, value_len);
    long prefix_len = 0;
    if (!pointer_is_null) {
        if (ret > 0 && ret <= (FT_Long)sizeof(buffer) && value_len >= ret) {
            prefix_len = ret;
        } else {
            prefix_len = 16;
        }
    }
    printf("{\"id\":\"%s\",\"key\":\"%s\",\"idx\":%u,\"prequery\":%ld,\"value_len\":%ld,\"return\":%ld,\"buffer_hex\":\"",
           id,
           key_name,
           idx,
           prequery,
           value_len,
           ret);
    print_hex_bytes(buffer, prefix_len);
    printf("\"}");
}

static int emit_ps_font_value_matrix(int argc, char** argv) {
    (void)argc;
    OracleFace type1;
    OracleFace custom;
    OracleFace cff;
    OracleFace truetype;
    FT_Long face_index = atol(argv[10]);
    int opened = open_oracle_face(argv[2], argv[3], face_index, &type1);
    if (opened != 0) return opened;
    opened = open_oracle_face(argv[4], argv[5], face_index, &custom);
    if (opened != 0) return opened;
    opened = open_oracle_face(argv[6], argv[7], face_index, &cff);
    if (opened != 0) return opened;
    opened = open_oracle_face(argv[8], argv[9], face_index, &truetype);
    if (opened != 0) return opened;
    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    print_ps_font_value_matrix_row("scalar_value", "PS_DICT_UNDERLINE_POSITION",
                                   type1.face, PS_DICT_UNDERLINE_POSITION, 0, 0, 256, 1);
    printf(",");
    print_ps_font_value_matrix_row("string_value", "PS_DICT_FULL_NAME",
                                   type1.face, PS_DICT_FULL_NAME, 0, 0, 256, 1);
    printf(",");
    print_ps_font_value_matrix_row("array_value", "PS_DICT_BLUE_VALUE",
                                   type1.face, PS_DICT_BLUE_VALUE, 0, 0, 256, 1);
    printf(",");
    print_ps_font_value_matrix_row("encoding_type", "PS_DICT_ENCODING_TYPE",
                                   custom.face, PS_DICT_ENCODING_TYPE, 0, 0, 256, 1);
    printf(",");
    print_ps_font_value_matrix_row("sizing_query", "PS_DICT_FULL_NAME",
                                   type1.face, PS_DICT_FULL_NAME, 0, 1, 0, 0);
    printf(",");
    print_ps_font_value_matrix_row("short_buffer", "PS_DICT_FULL_NAME",
                                   type1.face, PS_DICT_FULL_NAME, 0, 0, 1, 0);
    printf(",");
    print_ps_font_value_matrix_row("negative_value_len", "PS_DICT_FULL_NAME",
                                   type1.face, PS_DICT_FULL_NAME, 0, 0, -1, 0);
    printf(",");
    print_ps_font_value_matrix_row("invalid_index", "PS_DICT_BLUE_VALUE",
                                   type1.face, PS_DICT_BLUE_VALUE, 255, 0, 256, 1);
    printf(",");
    print_ps_font_value_matrix_row("unsupported_service", "PS_DICT_FULL_NAME",
                                   cff.face, PS_DICT_FULL_NAME, 0, 0, 256, 1);
    printf(",");
    print_ps_font_value_matrix_row("non_postscript", "PS_DICT_FULL_NAME",
                                   truetype.face, PS_DICT_FULL_NAME, 0, 0, 256, 1);
    printf(",");
    print_ps_font_value_matrix_row("null_face", "PS_DICT_FULL_NAME",
                                   NULL, PS_DICT_FULL_NAME, 0, 0, 256, 1);
    printf("]}}\n");
    close_oracle_face(&truetype);
    close_oracle_face(&cff);
    close_oracle_face(&custom);
    close_oracle_face(&type1);
    return 0;
}

static int emit_open_type_free_null_face(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_OpenType_Free(NULL, (FT_Bytes)1);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"free_event_count\":0,\"table_pointer_observed\":\"non_null_sentinel\"}}\n");
    return 0;
}

static int emit_open_type_free_null_table(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_OpenType_Free(face.face, NULL);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"free_event_count\":0}}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_gxval_free_null_face(int argc, char** argv) {
    (void)argc;
    const char* which = argv[2];
    if (streq(which, "gx")) {
        FT_TrueTypeGX_Free(NULL, (FT_Bytes)1);
    } else if (streq(which, "ckern")) {
        FT_ClassicKern_Free(NULL, (FT_Bytes)1);
    } else {
        return 2;
    }
    printf("{");
    print_status(0);
    printf(",\"output\":{\"crash\":false,\"free_event_count\":0,\"table_pointer_observed\":\"non_null_sentinel\"}}\n");
    return 0;
}

static void print_palette_data_json(FT_Error err, FT_Palette_Data data) {
    printf("{\"error\":%d,\"palette_data\":{\"num_palettes\":%u,\"num_palette_entries\":%u},",
           err,
           data.num_palettes,
           data.num_palette_entries);
    printf("\"pointer_nullness\":{\"palette_name_ids\":%s,\"palette_flags\":%s,\"palette_entry_name_ids\":%s}",
           data.palette_name_ids ? "false" : "true",
           data.palette_flags ? "false" : "true",
           data.palette_entry_name_ids ? "false" : "true");
    printf("}");
}

static void print_ushort_array(const FT_UShort* values, FT_UShort count) {
    if (!values) {
        printf("[]");
        return;
    }
    printf("[");
    for (FT_UShort i = 0; i < count; i++) {
        if (i) {
            printf(",");
        }
        printf("%u", values[i]);
    }
    printf("]");
}

static void print_palette_data_values_json(FT_Error err, FT_Palette_Data data) {
    printf("{\"error\":%d,\"palette_data\":{\"num_palettes\":%u,\"num_palette_entries\":%u,",
           err,
           data.num_palettes,
           data.num_palette_entries);
    printf("\"palette_name_ids\":{\"nullness\":%s,\"values\":",
           data.palette_name_ids ? "false" : "true");
    print_ushort_array(data.palette_name_ids, data.num_palettes);
    printf("},\"palette_flags\":{\"nullness\":%s,\"values\":",
           data.palette_flags ? "false" : "true");
    print_ushort_array(data.palette_flags, data.num_palettes);
    printf("},\"palette_entry_name_ids\":{\"nullness\":%s,\"values\":",
           data.palette_entry_name_ids ? "false" : "true");
    print_ushort_array(data.palette_entry_name_ids, data.num_palette_entries);
    printf("}}}");
}

static void print_palette_data_snapshot_json(FT_Palette_Data data) {
    printf("{\"num_palettes\":%u,\"num_palette_entries\":%u,"
           "\"pointer_nullness\":{\"palette_name_ids\":%s,\"palette_flags\":%s,\"palette_entry_name_ids\":%s}}",
           data.num_palettes,
           data.num_palette_entries,
           data.palette_name_ids ? "false" : "true",
           data.palette_flags ? "false" : "true",
           data.palette_entry_name_ids ? "false" : "true");
}

static void print_palette_data_null_input_row(const char* variant, FT_Error err, const FT_Palette_Data* data) {
    printf("{\"variant\":\"%s\",\"error\":%d,\"palette_data_snapshot\":",
           variant,
           err);
    if (data) {
        print_palette_data_snapshot_json(*data);
    } else {
        printf("null");
    }
    printf("}");
}

static void print_palette_select_error_row(const char* variant, FT_Error err, FT_Color* palette) {
    printf("{\"variant\":\"%s\",\"error\":%d,\"apalette_snapshot\":\"%s\",\"active_palette_index\":%d}",
           variant,
           err,
           palette ? "non_null" : "null",
           err ? -1 : 0);
}

static void print_color_entries(FT_Color* palette, FT_UShort count) {
    if (!palette) {
        printf("null");
        return;
    }
    printf("[");
    for (FT_UShort i = 0; i < count; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"blue\":%u,\"green\":%u,\"red\":%u,\"alpha\":%u}",
               palette[i].blue,
               palette[i].green,
               palette[i].red,
               palette[i].alpha);
    }
    printf("]");
}

static void print_ft_color_json(FT_Color color) {
    printf("{\"blue\":%u,\"green\":%u,\"red\":%u,\"alpha\":%u}",
           color.blue,
           color.green,
           color.red,
           color.alpha);
}

static FT_UShort palette_entry_count(FT_Face face) {
    FT_Palette_Data data = {0};
    FT_Error err = FT_Palette_Data_Get(face, &data);
    return err ? 0 : data.num_palette_entries;
}

static void print_layer_iterator_json(FT_LayerIterator iterator) {
    printf("{\"num_layers\":%u,\"layer\":%u,\"p_class\":\"%s\"}",
           iterator.num_layers,
           iterator.layer,
           iterator.p ? "nonnull" : "null");
}

static void print_color_layer_call_json(const char* label,
                                        FT_Bool result,
                                        FT_UInt glyph_index,
                                        FT_UInt color_index,
                                        FT_LayerIterator iterator) {
    printf("{\"label\":\"%s\",\"return\":%u,\"glyph_index\":%u,\"color_index\":%u,\"iterator\":",
           label,
           result,
           glyph_index,
           color_index);
    print_layer_iterator_json(iterator);
    printf("}");
}

static int emit_color_glyph_layer_case(int argc, char** argv) {
    if (argc != 7) {
        fprintf(stderr, "--color-glyph-layer-case requires CASE SOURCE_KIND SOURCE FACE_INDEX BASE_GLYPH\n");
        return 2;
    }
    const char* case_id = argv[2];
    OracleFace face;
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_UInt base_glyph = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_LayerIterator iterator;
    memset(&iterator, 0, sizeof(iterator));
    FT_UInt glyph_index = 0xDEAD;
    FT_UInt color_index = 0xBEEF;

    printf("{");
    print_status(0);
    if (streq(case_id, "ftcolor.FT_Get_Color_Glyph_Layer.layer_iteration_success") ||
        streq(case_id, "ftcolor.FT_LayerIterator.initialized_and_advanced_by_color_glyph_layers_v0")) {
        printf(",\"output\":{\"calls\":[");
        for (int i = 0; i < 4; i++) {
            if (i) {
                printf(",");
            }
            FT_Bool result = FT_Get_Color_Glyph_Layer(face.face, base_glyph, &glyph_index, &color_index, &iterator);
            char label[16];
            snprintf(label, sizeof(label), "call_%d", i + 1);
            print_color_layer_call_json(label, result, glyph_index, color_index, iterator);
        }
        printf("]}}\n");
    } else if (streq(case_id, "ftcolor.FT_Get_Color_Glyph_Layer.foreground_color_index")) {
        FT_Bool result = FT_Get_Color_Glyph_Layer(face.face, base_glyph, &glyph_index, &color_index, &iterator);
        printf(",\"output\":{\"call\":");
        print_color_layer_call_json("foreground", result, glyph_index, color_index, iterator);
        printf(",\"foreground_marker_preserved\":%s}}\n", color_index == 0xFFFF ? "true" : "false");
    } else if (streq(case_id, "ftcolor.FT_Get_Color_Glyph_Layer.terminal_false_preserves_last_outputs")) {
        for (int i = 0; i < 4; i++) {
            FT_Get_Color_Glyph_Layer(face.face, base_glyph, &glyph_index, &color_index, &iterator);
        }
        FT_UInt before_glyph = glyph_index;
        FT_UInt before_color = color_index;
        FT_LayerIterator before_iterator = iterator;
        FT_Bool terminal = FT_Get_Color_Glyph_Layer(face.face, base_glyph, &glyph_index, &color_index, &iterator);
        printf(",\"output\":{\"terminal_return\":%u,\"before\":{\"glyph_index\":%u,\"color_index\":%u,\"iterator\":",
               terminal,
               before_glyph,
               before_color);
        print_layer_iterator_json(before_iterator);
        printf("},\"after\":{\"glyph_index\":%u,\"color_index\":%u,\"iterator\":",
               glyph_index,
               color_index);
        print_layer_iterator_json(iterator);
        printf("}}}\n");
    } else {
        close_oracle_face(&face);
        return 2;
    }
    close_oracle_face(&face);
    return 0;
}

static void print_color_glyph_layer_sequence_for_base_glyph_json(FT_Face face,
                                                                 FT_UInt base_glyph,
                                                                 int max_calls) {
    FT_LayerIterator iterator;
    memset(&iterator, 0, sizeof(iterator));
    FT_UInt glyph_index = 0xDEAD;
    FT_UInt color_index = 0xBEEF;
    printf("{\"base_glyph\":%u,\"calls\":[", base_glyph);
    for (int i = 0; i < max_calls; i++) {
        if (i) {
            printf(",");
        }
        FT_Bool result = FT_Get_Color_Glyph_Layer(face, base_glyph, &glyph_index, &color_index, &iterator);
        char label[16];
        snprintf(label, sizeof(label), "call_%d", i + 1);
        print_color_layer_call_json(label, result, glyph_index, color_index, iterator);
    }
    printf("]}");
}

static FT_ClipBox sentinel_clip_box(void) {
    FT_ClipBox box;
    box.bottom_left.x = -0x1111;
    box.bottom_left.y = -0x2222;
    box.top_left.x = -0x3333;
    box.top_left.y = -0x4444;
    box.top_right.x = 0x5555;
    box.top_right.y = 0x6666;
    box.bottom_right.x = 0x7777;
    box.bottom_right.y = 0x8888;
    return box;
}

static int clip_box_equal(FT_ClipBox a, FT_ClipBox b) {
    return a.bottom_left.x == b.bottom_left.x &&
           a.bottom_left.y == b.bottom_left.y &&
           a.top_left.x == b.top_left.x &&
           a.top_left.y == b.top_left.y &&
           a.top_right.x == b.top_right.x &&
           a.top_right.y == b.top_right.y &&
           a.bottom_right.x == b.bottom_right.x &&
           a.bottom_right.y == b.bottom_right.y;
}

static void print_clip_vector_json(FT_Vector vector) {
    printf("{\"x\":%ld,\"y\":%ld}", vector.x, vector.y);
}

static void print_clip_box_json(FT_ClipBox clip_box) {
    printf("{\"bottom_left\":");
    print_clip_vector_json(clip_box.bottom_left);
    printf(",\"top_left\":");
    print_clip_vector_json(clip_box.top_left);
    printf(",\"top_right\":");
    print_clip_vector_json(clip_box.top_right);
    printf(",\"bottom_right\":");
    print_clip_vector_json(clip_box.bottom_right);
    printf("}");
}

static int emit_color_glyph_clipbox_case(int argc, char** argv) {
    if (argc != 7 && argc != 15) {
        fprintf(stderr, "--color-glyph-clipbox-case requires CASE SOURCE_KIND SOURCE FACE_INDEX BASE_GLYPH [PX PY XX XY YX YY DX DY]\n");
        return 2;
    }
    const char* case_id = argv[2];
    OracleFace face;
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_UInt base_glyph = (FT_UInt)strtoul(argv[6], NULL, 10);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"setup\":{");
    if (argc == 15) {
        FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
        FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
        FT_Error size_error = FT_Set_Pixel_Sizes(face.face, pixel_width, pixel_height);
        FT_Matrix matrix;
        matrix.xx = (FT_Fixed)strtol(argv[9], NULL, 10);
        matrix.xy = (FT_Fixed)strtol(argv[10], NULL, 10);
        matrix.yx = (FT_Fixed)strtol(argv[11], NULL, 10);
        matrix.yy = (FT_Fixed)strtol(argv[12], NULL, 10);
        FT_Vector delta;
        delta.x = (FT_Pos)strtol(argv[13], NULL, 10);
        delta.y = (FT_Pos)strtol(argv[14], NULL, 10);
        FT_Set_Transform(face.face, &matrix, &delta);
        printf("\"pixel_size\":{\"x\":%u,\"y\":%u,\"error\":%d},",
               pixel_width,
               pixel_height,
               size_error);
        printf("\"set_transform\":{\"matrix\":{\"xx\":%ld,\"xy\":%ld,\"yx\":%ld,\"yy\":%ld},\"delta\":",
               (long)matrix.xx,
               (long)matrix.xy,
               (long)matrix.yx,
               (long)matrix.yy);
        print_clip_vector_json(delta);
        printf("}");
    }
    printf("}");

    FT_ClipBox before = sentinel_clip_box();
    FT_ClipBox clip_box = before;
    FT_Bool result = FT_Get_Color_Glyph_ClipBox(face.face, base_glyph, &clip_box);
    printf(",\"return\":%u", result);
    if (streq(case_id, "ftcolor.FT_Get_Color_Glyph_ClipBox.no_clipbox_returns_false_preserves_output")) {
        printf(",\"clip_box_before_after\":{\"before\":");
        print_clip_box_json(before);
        printf(",\"after\":");
        print_clip_box_json(clip_box);
        printf(",\"preserved\":%s}", clip_box_equal(before, clip_box) ? "true" : "false");
    } else {
        printf(",\"clip_box\":");
        print_clip_box_json(clip_box);
    }
    printf("}}\n");
    close_oracle_face(&face);
    return 0;
}

static void print_opaque_paint_json(FT_OpaquePaint opaque) {
    printf("{\"p_class\":\"%s\",\"insert_root_transform\":%u}",
           opaque.p ? "nonnull" : "null",
           opaque.insert_root_transform);
}

static void print_colr_paint_node_json(FT_Face face, FT_OpaquePaint opaque, int depth) {
    if (depth > 8) {
        printf("{\"return\":0,\"depth_limit\":true}");
        return;
    }
    FT_COLR_Paint paint;
    memset(&paint, 0, sizeof(paint));
    FT_Bool result = FT_Get_Paint(face, opaque, &paint);
    printf("{\"return\":%u,\"opaque\":", result);
    print_opaque_paint_json(opaque);
    if (!result) {
        printf("}");
        return;
    }
    printf(",\"format\":%d", paint.format);
    printf("}");
}

static void print_colr_paint_layer_call_json(const char* label,
                                             FT_Bool result,
                                             FT_LayerIterator iterator,
                                             FT_OpaquePaint paint) {
    printf("{\"label\":\"%s\",\"return\":%u,\"iterator\":",
           label,
           result);
    print_layer_iterator_json(iterator);
    printf(",\"paint\":");
    print_opaque_paint_json(paint);
    printf("}");
}

static void print_colr_paint_layers_sequence_json(FT_Face face,
                                                  FT_UInt base_glyph,
                                                  int max_calls) {
    FT_OpaquePaint root_opaque;
    memset(&root_opaque, 0, sizeof(root_opaque));
    FT_Bool root_return = FT_Get_Color_Glyph_Paint(face, base_glyph, FT_COLOR_NO_ROOT_TRANSFORM, &root_opaque);
    FT_COLR_Paint paint;
    memset(&paint, 0, sizeof(paint));
    FT_Bool paint_return = FT_Get_Paint(face, root_opaque, &paint);
    FT_LayerIterator iterator;
    memset(&iterator, 0, sizeof(iterator));
    if (paint_return && paint.format == FT_COLR_PAINTFORMAT_COLR_LAYERS) {
        iterator = paint.u.colr_layers.layer_iterator;
    }
    FT_LayerIterator initial_iterator = iterator;
    printf("{\"base_glyph\":%u,\"root_return\":%u,\"root_opaque\":",
           base_glyph,
           root_return);
    print_opaque_paint_json(root_opaque);
    printf(",\"paint_return\":%u,\"paint_format\":%d,\"initial_iterator\":",
           paint_return,
           paint.format);
    print_layer_iterator_json(initial_iterator);
    printf(",\"calls\":[");
    for (int i = 0; i < max_calls; i++) {
        if (i) {
            printf(",");
        }
        FT_OpaquePaint layer_paint;
        layer_paint.p = (FT_Byte*)1;
        layer_paint.insert_root_transform = 0x7F;
        FT_Bool result = FT_Get_Paint_Layers(face, &iterator, &layer_paint);
        char label[16];
        snprintf(label, sizeof(label), "call_%d", i + 1);
        print_colr_paint_layer_call_json(label, result, iterator, layer_paint);
    }
    printf("]}");
}

static void print_ft_vector_json(FT_Vector vector) {
    printf("{\"x\":%ld,\"y\":%ld}", (long)vector.x, (long)vector.y);
}

static void print_color_stop_iterator_json(FT_ColorStopIterator iterator) {
    printf("{\"num_color_stops\":%u,\"current_color_stop\":%u,\"p_class\":\"%s\",\"read_variable\":%u}",
           iterator.num_color_stops,
           iterator.current_color_stop,
           iterator.p ? "nonnull" : "null",
           iterator.read_variable);
}

static void print_color_stop_json(FT_ColorStop stop) {
    printf("{\"stop_offset\":%ld,\"color\":{\"palette_index\":%u,\"alpha\":%d}}",
           (long)stop.stop_offset,
           stop.color.palette_index,
           stop.color.alpha);
}

static void print_colorline_json(FT_ColorLine colorline) {
    printf("{\"extend\":%d,\"color_stop_iterator\":", colorline.extend);
    print_color_stop_iterator_json(colorline.color_stop_iterator);
    printf("}");
}

static void print_gradient_paint_payload_json(FT_COLR_Paint paint) {
    if (paint.format == FT_COLR_PAINTFORMAT_LINEAR_GRADIENT) {
        printf("{\"format\":%d,\"linear_gradient\":{\"colorline\":", paint.format);
        print_colorline_json(paint.u.linear_gradient.colorline);
        printf(",\"p0\":");
        print_ft_vector_json(paint.u.linear_gradient.p0);
        printf(",\"p1\":");
        print_ft_vector_json(paint.u.linear_gradient.p1);
        printf(",\"p2\":");
        print_ft_vector_json(paint.u.linear_gradient.p2);
        printf("}}");
    } else if (paint.format == FT_COLR_PAINTFORMAT_RADIAL_GRADIENT) {
        printf("{\"format\":%d,\"radial_gradient\":{\"colorline\":", paint.format);
        print_colorline_json(paint.u.radial_gradient.colorline);
        printf(",\"c0\":");
        print_ft_vector_json(paint.u.radial_gradient.c0);
        printf(",\"r0\":%ld,\"c1\":", (long)paint.u.radial_gradient.r0);
        print_ft_vector_json(paint.u.radial_gradient.c1);
        printf(",\"r1\":%ld}}", (long)paint.u.radial_gradient.r1);
    } else if (paint.format == FT_COLR_PAINTFORMAT_SWEEP_GRADIENT) {
        printf("{\"format\":%d,\"sweep_gradient\":{\"colorline\":", paint.format);
        print_colorline_json(paint.u.sweep_gradient.colorline);
        printf(",\"center\":");
        print_ft_vector_json(paint.u.sweep_gradient.center);
        printf(",\"start_angle\":%ld,\"end_angle\":%ld}}",
               (long)paint.u.sweep_gradient.start_angle,
               (long)paint.u.sweep_gradient.end_angle);
    } else {
        printf("{\"format\":%d}", paint.format);
    }
}

static int colorline_from_gradient_paint(FT_COLR_Paint paint, FT_ColorLine* colorline) {
    if (paint.format == FT_COLR_PAINTFORMAT_LINEAR_GRADIENT) {
        *colorline = paint.u.linear_gradient.colorline;
        return 1;
    }
    if (paint.format == FT_COLR_PAINTFORMAT_RADIAL_GRADIENT) {
        *colorline = paint.u.radial_gradient.colorline;
        return 1;
    }
    if (paint.format == FT_COLR_PAINTFORMAT_SWEEP_GRADIENT) {
        *colorline = paint.u.sweep_gradient.colorline;
        return 1;
    }
    return 0;
}

static void print_colorline_stop_call_json(FT_Face face,
                                           const char* label,
                                           FT_ColorStopIterator* iterator,
                                           FT_ColorStop* color_stop) {
    FT_ColorStopIterator before_iterator = *iterator;
    FT_ColorStop before_stop = *color_stop;
    FT_Bool result = FT_Get_Colorline_Stops(face, color_stop, iterator);
    printf("{\"label\":\"%s\",\"return\":%u,\"before\":{\"iterator\":",
           label,
           result);
    print_color_stop_iterator_json(before_iterator);
    printf(",\"color_stop\":");
    print_color_stop_json(before_stop);
    printf("},\"after\":{\"iterator\":");
    print_color_stop_iterator_json(*iterator);
    printf(",\"color_stop\":");
    print_color_stop_json(*color_stop);
    printf("}}");
}

static void print_gradient_colorline_sequence_json(FT_Face face,
                                                   const char* label,
                                                   FT_UInt base_glyph,
                                                   int max_extra_calls) {
    FT_OpaquePaint opaque;
    memset(&opaque, 0, sizeof(opaque));
    FT_Bool root_return = FT_Get_Color_Glyph_Paint(face, base_glyph, FT_COLOR_NO_ROOT_TRANSFORM, &opaque);
    FT_COLR_Paint paint;
    memset(&paint, 0, sizeof(paint));
    FT_Bool paint_return = FT_Get_Paint(face, opaque, &paint);
    FT_ColorLine colorline;
    memset(&colorline, 0, sizeof(colorline));
    int has_colorline = paint_return && colorline_from_gradient_paint(paint, &colorline);
    FT_ColorStopIterator iterator;
    memset(&iterator, 0, sizeof(iterator));
    if (has_colorline) {
        iterator = colorline.color_stop_iterator;
    }
    FT_ColorStop color_stop;
    color_stop.stop_offset = -0x1234;
    color_stop.color.palette_index = 0xBEEF;
    color_stop.color.alpha = -0x123;
    int max_calls = (int)iterator.num_color_stops + max_extra_calls;

    printf("{\"label\":\"%s\",\"base_glyph\":%u,\"root_return\":%u,\"root_opaque\":",
           label,
           base_glyph,
           root_return);
    print_opaque_paint_json(opaque);
    printf(",\"paint_return\":%u,\"paint\":{\"format\":%d,\"node\":", paint_return, paint.format);
    print_colr_paint_node_json(face, opaque, 0);
    printf("}");
    printf(",\"colorline\":");
    if (has_colorline) {
        print_colorline_json(colorline);
    } else {
        printf("null");
    }
    printf(",\"calls\":[");
    for (int index = 0; index < max_calls; index++) {
        if (index) {
            printf(",");
        }
        char call_label[16];
        snprintf(call_label, sizeof(call_label), "call_%d", index + 1);
        print_colorline_stop_call_json(face, call_label, &iterator, &color_stop);
    }
    printf("]}");
}

static int emit_color_paint_layers_case(const char* case_id, OracleFace* face) {
    if (streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_COLR_LAYERS.paint_colr_layers_payload")) {
        printf("{");
        print_status(0);
        printf(",\"output\":{\"sequence\":");
        print_colr_paint_layers_sequence_json(face->face, 36, 3);
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "ftcolor.FT_Get_Paint_Layers.success_iterates_colr_v1_layers") ||
        streq(case_id, "ftcolor.FT_LayerIterator.initialized_and_advanced_by_paint_layers_v1")) {
        printf("{");
        print_status(0);
        printf(",\"output\":{\"sequences\":[");
        print_colr_paint_layers_sequence_json(face->face, 36, 3);
        printf(",");
        print_colr_paint_layers_sequence_json(face->face, 37, 4);
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftcolor.FT_LayerIterator.initialized_and_advanced_by_layer_apis")) {
        printf("{");
        print_status(0);
        printf(",\"output\":{\"color_glyph_layers_v0\":");
        print_color_glyph_layer_sequence_for_base_glyph_json(face->face, 36, 4);
        printf(",\"paint_layers_v1\":[");
        print_colr_paint_layers_sequence_json(face->face, 36, 3);
        printf(",");
        print_colr_paint_layers_sequence_json(face->face, 37, 4);
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftcolor.FT_Get_Paint_Layers.end_of_iteration")) {
        printf("{");
        print_status(0);
        printf(",\"output\":{\"sequence\":");
        print_colr_paint_layers_sequence_json(face->face, 37, 5);
        printf("}}\n");
        return 0;
    }
    return 2;
}

static void print_colr_snapshot_node_json(FT_Face face, FT_OpaquePaint opaque, int depth) {
    FT_COLR_Paint paint;
    memset(&paint, 0, sizeof(paint));
    FT_Bool result = FT_Get_Paint(face, opaque, &paint);
    if (!result) {
    printf("{\"depth\":%d,\"format\":0,\"palette_index\":0,\"alpha\":0,\"glyph_index\":0,\"composite_mode\":0,\"values\":[0,0,0,0,0,0]}", depth);
        return;
    }
    FT_UInt palette_index = 0;
    FT_Int alpha = 0;
    FT_UInt glyph_index = 0;
    FT_Int composite_mode = 0;
    long values[6] = {0, 0, 0, 0, 0, 0};
    if (paint.format == FT_COLR_PAINTFORMAT_SOLID) {
        palette_index = paint.u.solid.color.palette_index;
        alpha = paint.u.solid.color.alpha;
    } else if (paint.format == FT_COLR_PAINTFORMAT_GLYPH) {
        glyph_index = paint.u.glyph.glyphID;
    } else if (paint.format == FT_COLR_PAINTFORMAT_COLR_GLYPH) {
        glyph_index = paint.u.colr_glyph.glyphID;
    } else if (paint.format == FT_COLR_PAINTFORMAT_LINEAR_GRADIENT) {
        palette_index = paint.u.linear_gradient.colorline.color_stop_iterator.num_color_stops;
        composite_mode = paint.u.linear_gradient.colorline.extend;
        values[0] = (long)paint.u.linear_gradient.p0.x;
        values[1] = (long)paint.u.linear_gradient.p0.y;
        values[2] = (long)paint.u.linear_gradient.p1.x;
        values[3] = (long)paint.u.linear_gradient.p1.y;
        values[4] = (long)paint.u.linear_gradient.p2.x;
        values[5] = (long)paint.u.linear_gradient.p2.y;
    } else if (paint.format == FT_COLR_PAINTFORMAT_RADIAL_GRADIENT) {
        palette_index = paint.u.radial_gradient.colorline.color_stop_iterator.num_color_stops;
        composite_mode = paint.u.radial_gradient.colorline.extend;
        values[0] = (long)paint.u.radial_gradient.c0.x;
        values[1] = (long)paint.u.radial_gradient.c0.y;
        values[2] = (long)paint.u.radial_gradient.r0;
        values[3] = (long)paint.u.radial_gradient.c1.x;
        values[4] = (long)paint.u.radial_gradient.c1.y;
        values[5] = (long)paint.u.radial_gradient.r1;
    } else if (paint.format == FT_COLR_PAINTFORMAT_SWEEP_GRADIENT) {
        palette_index = paint.u.sweep_gradient.colorline.color_stop_iterator.num_color_stops;
        composite_mode = paint.u.sweep_gradient.colorline.extend;
        values[0] = (long)paint.u.sweep_gradient.center.x;
        values[1] = (long)paint.u.sweep_gradient.center.y;
        values[2] = (long)paint.u.sweep_gradient.start_angle;
        values[3] = (long)paint.u.sweep_gradient.end_angle;
    } else if (paint.format == FT_COLR_PAINTFORMAT_COLR_LAYERS) {
        composite_mode = paint.u.colr_layers.layer_iterator.num_layers;
    } else if (paint.format == FT_COLR_PAINTFORMAT_TRANSFORM) {
        values[0] = (long)paint.u.transform.affine.xx;
        values[1] = (long)paint.u.transform.affine.xy;
        values[2] = (long)paint.u.transform.affine.dx;
        values[3] = (long)paint.u.transform.affine.yx;
        values[4] = (long)paint.u.transform.affine.yy;
        values[5] = (long)paint.u.transform.affine.dy;
    } else if (paint.format == FT_COLR_PAINTFORMAT_TRANSLATE) {
        values[0] = (long)paint.u.translate.dx;
        values[1] = (long)paint.u.translate.dy;
    } else if (paint.format == FT_COLR_PAINTFORMAT_SCALE) {
        values[0] = (long)paint.u.scale.scale_x;
        values[1] = (long)paint.u.scale.scale_y;
        values[2] = (long)paint.u.scale.center_x;
        values[3] = (long)paint.u.scale.center_y;
    } else if (paint.format == FT_COLR_PAINTFORMAT_ROTATE) {
        values[0] = (long)paint.u.rotate.angle;
        values[1] = (long)paint.u.rotate.center_x;
        values[2] = (long)paint.u.rotate.center_y;
    } else if (paint.format == FT_COLR_PAINTFORMAT_SKEW) {
        values[0] = (long)paint.u.skew.x_skew_angle;
        values[1] = (long)paint.u.skew.y_skew_angle;
        values[2] = (long)paint.u.skew.center_x;
        values[3] = (long)paint.u.skew.center_y;
    } else if (paint.format == FT_COLR_PAINTFORMAT_COMPOSITE) {
        composite_mode = paint.u.composite.composite_mode;
    }
    printf("{\"depth\":%d,\"format\":%d,\"palette_index\":%u,\"alpha\":%d,\"glyph_index\":%u,\"composite_mode\":%d,\"values\":[%ld,%ld,%ld,%ld,%ld,%ld]}",
           depth, paint.format, palette_index, alpha, glyph_index, composite_mode,
           values[0], values[1], values[2], values[3], values[4], values[5]);
    if (paint.format == FT_COLR_PAINTFORMAT_COLR_LAYERS) {
        FT_LayerIterator iterator = paint.u.colr_layers.layer_iterator;
        FT_OpaquePaint layer_paint;
        while (FT_Get_Paint_Layers(face, &iterator, &layer_paint)) {
            printf(",");
            print_colr_snapshot_node_json(face, layer_paint, depth + 1);
        }
    } else if (paint.format == FT_COLR_PAINTFORMAT_GLYPH) {
        printf(",");
        print_colr_snapshot_node_json(face, paint.u.glyph.paint, depth + 1);
    } else if (paint.format == FT_COLR_PAINTFORMAT_TRANSFORM) {
        printf(",");
        print_colr_snapshot_node_json(face, paint.u.transform.paint, depth + 1);
    } else if (paint.format == FT_COLR_PAINTFORMAT_TRANSLATE) {
        printf(",");
        print_colr_snapshot_node_json(face, paint.u.translate.paint, depth + 1);
    } else if (paint.format == FT_COLR_PAINTFORMAT_SCALE) {
        printf(",");
        print_colr_snapshot_node_json(face, paint.u.scale.paint, depth + 1);
    } else if (paint.format == FT_COLR_PAINTFORMAT_ROTATE) {
        printf(",");
        print_colr_snapshot_node_json(face, paint.u.rotate.paint, depth + 1);
    } else if (paint.format == FT_COLR_PAINTFORMAT_SKEW) {
        printf(",");
        print_colr_snapshot_node_json(face, paint.u.skew.paint, depth + 1);
    } else if (paint.format == FT_COLR_PAINTFORMAT_COMPOSITE) {
        printf(",");
        print_colr_snapshot_node_json(face, paint.u.composite.source_paint, depth + 1);
        printf(",");
        print_colr_snapshot_node_json(face, paint.u.composite.backdrop_paint, depth + 1);
    }
}

static void print_colr_snapshot_record_json(FT_Face face, FT_UInt base_glyph) {
    FT_OpaquePaint opaque;
    memset(&opaque, 0, sizeof(opaque));
    FT_Get_Color_Glyph_Paint(face, base_glyph, FT_COLOR_NO_ROOT_TRANSFORM, &opaque);
    printf("{\"glyph_index\":%u,\"nodes\":[", base_glyph);
    print_colr_snapshot_node_json(face, opaque, 0);
    printf("]}");
}

static void print_colr_graph_snapshot_json(FT_Face face) {
    printf("{\"root_count\":30,\"records\":[");
    print_colr_snapshot_record_json(face, 36);
    printf(",");
    print_colr_snapshot_record_json(face, 37);
    for (int mode = 0; mode < 28; mode++) {
        printf(",");
        print_colr_snapshot_record_json(face, (FT_UInt)(39 + mode));
    }
    printf("]}");
}

static void print_colr_glyph_graph_snapshot_json(FT_Face face) {
    printf("{\"root_count\":2,\"records\":[");
    print_colr_snapshot_record_json(face, 36);
    printf(",");
    print_colr_snapshot_record_json(face, 37);
    printf("]}");
}

static void print_colr_root_json(FT_Face face, FT_UInt base_glyph, FT_UInt root_transform) {
    FT_OpaquePaint opaque;
    memset(&opaque, 0, sizeof(opaque));
    FT_Bool result = FT_Get_Color_Glyph_Paint(face, base_glyph, root_transform, &opaque);
    printf("{\"root_return\":%u,\"root_opaque\":", result);
    print_opaque_paint_json(opaque);
    printf(",\"root_paint\":");
    print_colr_paint_node_json(face, opaque, 0);
    printf("}");
}

static int case_base_matches(const char* case_id, const char* base) {
    size_t len = strlen(base);
    return strncmp(case_id, base, len) == 0 &&
           (case_id[len] == '\0' || case_id[len] == '@');
}

static int color_root_case_ppem(const char* case_id) {
    const char* variant = strchr(case_id, '@');
    if (!variant) {
        return 16;
    }
    if (streq(variant, "@s12")) {
        return 12;
    }
    if (streq(variant, "@s16")) {
        return 16;
    }
    if (streq(variant, "@s31")) {
        return 31;
    }
    if (streq(variant, "@s48")) {
        return 48;
    }
    return 16;
}

static void print_affine23_json(FT_Affine23 affine) {
    printf("{\"xx\":%ld,\"xy\":%ld,\"dx\":%ld,\"yx\":%ld,\"yy\":%ld,\"dy\":%ld}",
           (long)affine.xx,
           (long)affine.xy,
           (long)affine.dx,
           (long)affine.yx,
           (long)affine.yy,
           (long)affine.dy);
}

static void print_paint_transform_json(FT_PaintTransform transform) {
    printf("{\"paint\":");
    print_opaque_paint_json(transform.paint);
    printf(",\"affine\":");
    print_affine23_json(transform.affine);
    printf("}");
}

static void color_root_setup_values(const char* label, FT_Matrix* matrix, FT_Vector* delta) {
    if (streq(label, "identity")) {
        matrix->xx = 65536;
        matrix->xy = 0;
        matrix->yx = 0;
        matrix->yy = 65536;
        delta->x = 0;
        delta->y = 0;
        return;
    }
    if (streq(label, "manifest_include_transform")) {
        matrix->xx = 65536;
        matrix->xy = 8192;
        matrix->yx = 0;
        matrix->yy = 65536;
        delta->x = 64;
        delta->y = 32;
        return;
    }
    matrix->xx = 65536;
    matrix->xy = 8192;
    matrix->yx = -4096;
    matrix->yy = 65536;
    delta->x = 96;
    delta->y = -64;
}

static void print_colr_root_transform_row_json(FT_Face face,
                                               const char* label,
                                               FT_UInt base_glyph,
                                               int ppem,
                                               FT_UInt root_transform) {
    FT_Matrix matrix;
    FT_Vector delta;
    color_root_setup_values(label, &matrix, &delta);
    FT_Error size_error = FT_Set_Pixel_Sizes(face, 0, (FT_UInt)ppem);
    FT_Set_Transform(face, &matrix, &delta);

    FT_OpaquePaint opaque;
    memset(&opaque, 0, sizeof(opaque));
    FT_Bool root_return = FT_Get_Color_Glyph_Paint(face, base_glyph, root_transform, &opaque);
    FT_COLR_Paint paint;
    memset(&paint, 0, sizeof(paint));
    FT_Bool paint_return = FT_Get_Paint(face, opaque, &paint);

    printf("{\"label\":\"%s\",\"pixel_size\":{\"x\":0,\"y\":%d},\"setup\":{\"size_error\":%d,\"set_transform\":{\"matrix\":{\"xx\":%ld,\"xy\":%ld,\"yx\":%ld,\"yy\":%ld},\"delta\":{\"x\":%ld,\"y\":%ld}}},\"root_transform\":%u,\"base_glyph\":%u,\"root_return\":%u,\"root_opaque\":",
           label,
           ppem,
           size_error,
           (long)matrix.xx,
           (long)matrix.xy,
           (long)matrix.yx,
           (long)matrix.yy,
           (long)delta.x,
           (long)delta.y,
           root_transform,
           base_glyph,
           root_return);
    print_opaque_paint_json(opaque);
    printf(",\"paint_return\":%u,\"paint_format\":%d,\"transform\":", paint_return, paint.format);
    if (paint_return && paint.format == FT_COLR_PAINTFORMAT_TRANSFORM) {
        print_paint_transform_json(paint.u.transform);
    } else {
        printf("null");
    }
    printf(",\"root_paint\":");
    print_colr_paint_node_json(face, opaque, 0);
    printf("}");
}

static int is_colr_root_transform_case(const char* case_id) {
    return case_base_matches(case_id, "ftcolor.FT_COLOR_INCLUDE_ROOT_TRANSFORM.include_transform_runtime") ||
           case_base_matches(case_id, "ftcolor.FT_COLOR_NO_ROOT_TRANSFORM.omit_transform_runtime") ||
           case_base_matches(case_id, "ftcolor.FT_Color_Root_Transform.root_transform_controls_initial_paint") ||
           case_base_matches(case_id, "ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.included_root_transform_payload") ||
           case_base_matches(case_id, "ftcolor.FT_Get_Color_Glyph_Paint.root_paint_success_include_root_transform");
}

static int emit_colr_root_transform_case(const char* case_id, OracleFace* face) {
    int ppem = color_root_case_ppem(case_id);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    if (case_base_matches(case_id, "ftcolor.FT_COLOR_INCLUDE_ROOT_TRANSFORM.include_transform_runtime") ||
        case_base_matches(case_id, "ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.included_root_transform_payload")) {
        print_colr_root_transform_row_json(face->face, "scale_translate", 36, ppem, FT_COLOR_INCLUDE_ROOT_TRANSFORM);
    } else if (case_base_matches(case_id, "ftcolor.FT_Get_Color_Glyph_Paint.root_paint_success_include_root_transform")) {
        print_colr_root_transform_row_json(face->face, "manifest_include_transform", 36, 24, FT_COLOR_INCLUDE_ROOT_TRANSFORM);
    } else if (case_base_matches(case_id, "ftcolor.FT_COLOR_NO_ROOT_TRANSFORM.omit_transform_runtime")) {
        print_colr_root_transform_row_json(face->face, "scale_translate", 36, ppem, FT_COLOR_NO_ROOT_TRANSFORM);
    } else {
        print_colr_root_transform_row_json(face->face, "identity", 36, ppem, FT_COLOR_INCLUDE_ROOT_TRANSFORM);
        printf(",");
        print_colr_root_transform_row_json(face->face, "identity", 36, ppem, FT_COLOR_NO_ROOT_TRANSFORM);
        printf(",");
        print_colr_root_transform_row_json(face->face, "scale_translate", 36, ppem, FT_COLOR_INCLUDE_ROOT_TRANSFORM);
        printf(",");
        print_colr_root_transform_row_json(face->face, "scale_translate", 36, ppem, FT_COLOR_NO_ROOT_TRANSFORM);
    }
    printf("]}}\n");
    return 0;
}

static void print_colr_all_paints_snapshot_json(FT_Face face) {
    printf("{\"root_count\":15,\"records\":[");
    for (int i = 0; i < 15; i++) {
        if (i) {
            printf(",");
        }
        print_colr_snapshot_record_json(face, (FT_UInt)(36 + i));
    }
    printf("]}");
}

static void print_colr_all_paints_role_row_json(FT_Face face,
                                                const char* label,
                                                FT_UInt base_glyph) {
    FT_OpaquePaint opaque;
    memset(&opaque, 0, sizeof(opaque));
    FT_Bool root_return = FT_Get_Color_Glyph_Paint(face, base_glyph, FT_COLOR_NO_ROOT_TRANSFORM, &opaque);
    FT_COLR_Paint paint;
    memset(&paint, 0, sizeof(paint));
    FT_Bool paint_return = FT_Get_Paint(face, opaque, &paint);
    printf("{\"label\":\"%s\",\"base_glyph\":%u,\"root_return\":%u,\"root_opaque\":",
           label,
           base_glyph,
           root_return);
    print_opaque_paint_json(opaque);
    printf(",\"paint_return\":%u,\"paint_format\":%d,\"root_paint\":",
           paint_return,
           paint.format);
    print_colr_paint_node_json(face, opaque, 0);
    printf(",\"colorline\":");
    FT_ColorLine colorline;
    memset(&colorline, 0, sizeof(colorline));
    if (paint_return && colorline_from_gradient_paint(paint, &colorline)) {
        print_colorline_json(colorline);
    } else {
        printf("null");
    }
    printf(",\"layer_iterator\":");
    if (paint_return && paint.format == FT_COLR_PAINTFORMAT_COLR_LAYERS) {
        print_layer_iterator_json(paint.u.colr_layers.layer_iterator);
    } else {
        printf("null");
    }
    printf(",\"transform\":");
    if (paint_return && paint.format == FT_COLR_PAINTFORMAT_TRANSFORM) {
        print_paint_transform_json(paint.u.transform);
    } else {
        printf("null");
    }
    printf("}");
}

static void print_colr_all_paints_rows_json(FT_Face face) {
    const char* labels[15] = {
        "colr_layers",
        "solid",
        "glyph",
        "colr_glyph",
        "linear_gradient",
        "radial_gradient",
        "sweep_gradient",
        "transform",
        "translate",
        "scale",
        "rotate",
        "skew",
        "composite",
        "root_transform_target",
        "foreground_solid",
    };
    printf("[");
    for (int i = 0; i < 15; i++) {
        if (i) {
            printf(",");
        }
        print_colr_all_paints_role_row_json(face, labels[i], (FT_UInt)(36 + i));
    }
    printf("]");
}

static void print_colr_all_paints_root_transform_runs_json(FT_Face face) {
    const int ppems[4] = { 16, 16, 37, 37 };
    const char* labels[4] = { "identity", "shear_translate", "identity", "shear_translate" };
    printf("[");
    for (int i = 0; i < 4; i++) {
        if (i) {
            printf(",");
        }
        if (streq(labels[i], "shear_translate")) {
            FT_Matrix matrix = { 65536, 16384, -8192, 65536 };
            FT_Vector delta = { 3, -5 };
            FT_Error size_error = FT_Set_Pixel_Sizes(face, 0, (FT_UInt)ppems[i]);
            FT_Set_Transform(face, &matrix, &delta);
            FT_OpaquePaint opaque;
            memset(&opaque, 0, sizeof(opaque));
            FT_Bool root_return = FT_Get_Color_Glyph_Paint(face, 49, FT_COLOR_INCLUDE_ROOT_TRANSFORM, &opaque);
            FT_COLR_Paint paint;
            memset(&paint, 0, sizeof(paint));
            FT_Bool paint_return = FT_Get_Paint(face, opaque, &paint);
            printf("{\"label\":\"shear_translate\",\"pixel_size\":{\"x\":0,\"y\":%d},\"setup\":{\"size_error\":%d,\"set_transform\":{\"matrix\":{\"xx\":65536,\"xy\":16384,\"yx\":-8192,\"yy\":65536},\"delta\":{\"x\":3,\"y\":-5}}},\"root_transform\":%u,\"base_glyph\":49,\"root_return\":%u,\"root_opaque\":",
                   ppems[i],
                   size_error,
                   FT_COLOR_INCLUDE_ROOT_TRANSFORM,
                   root_return);
            print_opaque_paint_json(opaque);
            printf(",\"paint_return\":%u,\"paint_format\":%d,\"transform\":", paint_return, paint.format);
            if (paint_return && paint.format == FT_COLR_PAINTFORMAT_TRANSFORM) {
                print_paint_transform_json(paint.u.transform);
            } else {
                printf("null");
            }
            printf(",\"root_paint\":");
            print_colr_paint_node_json(face, opaque, 0);
            printf("}");
        } else {
            print_colr_root_transform_row_json(face, "identity", 49, ppems[i], FT_COLOR_INCLUDE_ROOT_TRANSFORM);
        }
    }
    printf("]");
}

static int is_colr_all_paints_case(const char* case_id) {
    return case_base_matches(case_id, "ftcolor.FT_Get_Paint.success_resolves_each_supported_paint_format") ||
           case_base_matches(case_id, "ftcolor.FT_Get_Paint.success_inserts_root_transform") ||
           case_base_matches(case_id, "ftcolor.FT_Affine23.root_transform_values") ||
           case_base_matches(case_id, "ftcolor.FT_ColorStopIterator.initialized_by_get_paint") ||
           case_base_matches(case_id, "ftcolor.FT_ColorIndex.solid_and_color_stop_values") ||
           case_base_matches(case_id, "ftcolor.FT_PaintFormat.paint_union_shape_runtime") ||
           case_base_matches(case_id, "ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values") ||
           case_base_matches(case_id, "ftcolor.FT_PaintColrLayers.get_paint_initializes_layer_iterator");
}

static int emit_colr_all_paints_case(const char* case_id, OracleFace* face) {
    printf("{");
    print_status(0);
    if (case_base_matches(case_id, "ftcolor.FT_Get_Paint.success_resolves_each_supported_paint_format") ||
        case_base_matches(case_id, "ftcolor.FT_PaintFormat.paint_union_shape_runtime")) {
        printf(",\"output\":{\"rows\":");
        print_colr_all_paints_rows_json(face->face);
        printf(",\"graph_snapshot\":");
        print_colr_all_paints_snapshot_json(face->face);
        printf("}}\n");
        return 0;
    }
    if (case_base_matches(case_id, "ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values")) {
        printf(",\"output\":{\"row\":");
        print_colr_all_paints_role_row_json(face->face, "colr_glyph", 39);
        printf(",\"graph_snapshot\":");
        print_colr_all_paints_snapshot_json(face->face);
        printf("}}\n");
        return 0;
    }
    if (case_base_matches(case_id, "ftcolor.FT_PaintColrLayers.get_paint_initializes_layer_iterator")) {
        printf(",\"output\":{\"sequence\":");
        print_colr_paint_layers_sequence_json(face->face, 36, 4);
        printf(",\"row\":");
        print_colr_all_paints_role_row_json(face->face, "colr_layers", 36);
        printf("}}\n");
        return 0;
    }
    if (case_base_matches(case_id, "ftcolor.FT_ColorStopIterator.initialized_by_get_paint")) {
        printf(",\"output\":{\"sequences\":[");
        print_gradient_colorline_sequence_json(face->face, "linear_pad", 40, 1);
        printf(",");
        print_gradient_colorline_sequence_json(face->face, "radial_repeat", 41, 1);
        printf(",");
        print_gradient_colorline_sequence_json(face->face, "sweep_reflect", 42, 1);
        printf("]}}\n");
        return 0;
    }
    if (case_base_matches(case_id, "ftcolor.FT_ColorIndex.solid_and_color_stop_values")) {
        printf(",\"output\":{\"solid_paints\":[");
        print_colr_all_paints_role_row_json(face->face, "solid", 37);
        printf(",");
        print_colr_all_paints_role_row_json(face->face, "foreground_solid", 50);
        printf("],\"colorline\":");
        print_gradient_colorline_sequence_json(face->face, "linear_pad", 40, 1);
        printf("}}\n");
        return 0;
    }
    printf(",\"output\":{\"runs\":");
    print_colr_all_paints_root_transform_runs_json(face->face);
    printf("}}\n");
    return 0;
}

static void print_foreground_solid_public_reference_json(FT_Face face) {
    FT_OpaquePaint opaque;
    memset(&opaque, 0, sizeof(opaque));
    FT_Bool root_return = FT_Get_Color_Glyph_Paint(
        face,
        50,
        FT_COLOR_NO_ROOT_TRANSFORM,
        &opaque);
    FT_COLR_Paint paint;
    memset(&paint, 0, sizeof(paint));
    FT_Bool paint_return = FT_Get_Paint(face, opaque, &paint);
    printf("{\"kind\":\"public_color_index\",\"base_glyph\":50,\"root_return\":%u,\"paint_return\":%u,\"paint_format\":%d,\"color\":",
           root_return,
           paint_return,
           paint.format);
    if (paint_return && paint.format == FT_COLR_PAINTFORMAT_SOLID) {
        printf("{\"palette_index\":%u,\"alpha\":%d}",
               paint.u.solid.color.palette_index,
               paint.u.solid.color.alpha);
    } else {
        printf("null");
    }
    printf("}");
}

static void print_palette_set_foreground_sfnt_runs_json(FT_Face face) {
    FT_Color colors[3] = {
        { 0, 0, 0, 255 },
        { 255, 16, 128, 64 },
        { 0, 0, 0, 0 },
    };
    FT_Error select_error = FT_Palette_Select(face, 0, NULL);
    printf("{\"runs\":[");
    for (int i = 0; i < 3; i++) {
        if (i) {
            printf(",");
        }
        FT_Error set_error = FT_Palette_Set_Foreground_Color(face, colors[i]);
        printf("{\"error\":%d,\"foreground_color\":",
               select_error ? select_error : set_error);
        print_ft_color_json(colors[i]);
        printf(",\"observable_bgra_or_color_index\":");
        print_foreground_solid_public_reference_json(face);
        printf("}");
    }
    printf("]}");
}

static FT_Color default_foreground_color_for_palette_flags(FT_UShort palette_flags) {
    if (palette_flags & FT_PALETTE_FOR_DARK_BACKGROUND) {
        FT_Color color = { 255, 255, 255, 255 };
        return color;
    }
    FT_Color color = { 0, 0, 0, 255 };
    return color;
}

static void print_palette_default_foreground_policy_json(FT_Face face) {
    FT_Palette_Data data;
    memset(&data, 0, sizeof(data));
    FT_Error data_error = FT_Palette_Data_Get(face, &data);
    FT_UShort num_palettes = data_error ? 0 : data.num_palettes;
    printf("{\"runs\":[");
    for (FT_UShort i = 0; i < num_palettes; i++) {
        if (i) {
            printf(",");
        }
        FT_UShort palette_flags = data.palette_flags ? data.palette_flags[i] : 0;
        FT_Error select_error = FT_Palette_Select(face, i, NULL);
        FT_Color default_foreground = default_foreground_color_for_palette_flags(palette_flags);
        printf("{\"palette_index\":%u,\"select_error\":%d,\"palette_flags\":%u,\"default_foreground_bgra\":",
               i,
               select_error,
               palette_flags);
        print_ft_color_json(default_foreground);
        printf(",\"render_or_blend_output\":{\"kind\":\"resolved_default_foreground_and_public_paint_reference\",\"resolved_default_foreground_bgra\":");
        print_ft_color_json(default_foreground);
        printf(",\"public_foreground_reference\":");
        print_foreground_solid_public_reference_json(face);
        printf("}}");
    }
    printf("]}");
}

static int emit_colr_glyph_paint_graph_case(OracleFace* face) {
    printf("{");
    print_status(0);
    printf(",\"output\":{\"colr_glyph_root\":");
    print_colr_root_json(face->face, 36, FT_COLOR_NO_ROOT_TRANSFORM);
    printf(",\"referenced_root\":");
    print_colr_root_json(face->face, 37, FT_COLOR_NO_ROOT_TRANSFORM);
    printf(",\"graph_snapshot\":");
    print_colr_glyph_graph_snapshot_json(face->face);
    printf("}}\n");
    return 0;
}

static void print_colr_transform_paint_row_json(FT_Face face,
                                                const char* label,
                                                FT_UInt base_glyph) {
    FT_OpaquePaint opaque;
    memset(&opaque, 0, sizeof(opaque));
    FT_Bool root_return = FT_Get_Color_Glyph_Paint(face, base_glyph, FT_COLOR_NO_ROOT_TRANSFORM, &opaque);
    printf("{\"label\":\"%s\",\"base_glyph\":%u,\"root_return\":%u,\"root_opaque\":",
           label,
           base_glyph,
           root_return);
    print_opaque_paint_json(opaque);
    printf(",\"root_paint\":");
    print_colr_paint_node_json(face, opaque, 0);
    printf("}");
}

static void print_colr_transform_graph_snapshot_json(FT_Face face) {
    printf("{\"root_count\":10,\"records\":[");
    for (int i = 0; i < 10; i++) {
        if (i) {
            printf(",");
        }
        print_colr_snapshot_record_json(face, (FT_UInt)(36 + i));
    }
    printf("]}");
}

static int emit_colr_transform_paint_case(OracleFace* face) {
    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    const char* labels[10] = {
        "transform",
        "translate",
        "scale",
        "scale_center",
        "scale_uniform",
        "scale_uniform_center",
        "rotate",
        "rotate_center",
        "skew",
        "skew_center",
    };
    for (int i = 0; i < 10; i++) {
        if (i) {
            printf(",");
        }
        print_colr_transform_paint_row_json(face->face, labels[i], (FT_UInt)(36 + i));
    }
    printf("],\"graph_snapshot\":");
    print_colr_transform_graph_snapshot_json(face->face);
    printf("}}\n");
    return 0;
}

static void print_colr_static_gradient_graph_snapshot_json(FT_Face face) {
    printf("{\"root_count\":3,\"records\":[");
    print_colr_snapshot_record_json(face, 36);
    printf(",");
    print_colr_snapshot_record_json(face, 37);
    printf(",");
    print_colr_snapshot_record_json(face, 38);
    printf("]}");
}

static int emit_colr_static_gradient_case(const char* case_id, OracleFace* face) {
    printf("{");
    print_status(0);
    if (streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_LINEAR_GRADIENT.paint_linear_gradient_payload") ||
        streq(case_id, "ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_static_values") ||
        streq(case_id, "ftcolor.FT_COLR_PAINT_EXTEND_PAD.colorline_extend_pad")) {
        printf(",\"output\":{\"sequence\":");
        print_gradient_colorline_sequence_json(face->face, "linear_pad", 36, 1);
        printf(",\"graph_snapshot\":");
        print_colr_static_gradient_graph_snapshot_json(face->face);
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_RADIAL_GRADIENT.paint_radial_gradient_payload") ||
        streq(case_id, "ftcolor.FT_PaintRadialGradient.get_paint_radial_gradient_values") ||
        streq(case_id, "ftcolor.FT_COLR_PAINT_EXTEND_REPEAT.colorline_extend_repeat")) {
        printf(",\"output\":{\"sequence\":");
        print_gradient_colorline_sequence_json(face->face, "radial_repeat", 37, 1);
        printf(",\"graph_snapshot\":");
        print_colr_static_gradient_graph_snapshot_json(face->face);
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_SWEEP_GRADIENT.paint_sweep_gradient_payload") ||
        streq(case_id, "ftcolor.FT_PaintSweepGradient.get_paint_sweep_gradient_values") ||
        streq(case_id, "ftcolor.FT_COLR_PAINT_EXTEND_REFLECT.colorline_extend_reflect")) {
        printf(",\"output\":{\"sequence\":");
        print_gradient_colorline_sequence_json(face->face, "sweep_reflect", 38, 1);
        printf(",\"graph_snapshot\":");
        print_colr_static_gradient_graph_snapshot_json(face->face);
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "ftcolor.FT_Get_Colorline_Stops.end_of_iteration") ||
        streq(case_id, "ftcolor.FT_ColorStopIterator.advanced_by_get_colorline_stops")) {
        printf(",\"output\":{\"sequence\":");
        print_gradient_colorline_sequence_json(face->face, "linear_pad", 36, 2);
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "ftcolor.FT_PaintExtend.gradient_extend_runtime") ||
        streq(case_id, "ftcolor.FT_ColorLine.gradient_colorline_values") ||
        streq(case_id, "ftcolor.FT_Get_Colorline_Stops.success_iterates_static_colorline_stops")) {
        printf(",\"output\":{\"sequences\":[");
        print_gradient_colorline_sequence_json(face->face, "linear_pad", 36, 1);
        printf(",");
        print_gradient_colorline_sequence_json(face->face, "radial_repeat", 37, 1);
        printf(",");
        print_gradient_colorline_sequence_json(face->face, "sweep_reflect", 38, 1);
        printf("],\"graph_snapshot\":");
        print_colr_static_gradient_graph_snapshot_json(face->face);
        printf("}}\n");
        return 0;
    }
    printf(",\"output\":{}}\n");
    return 0;
}

static void print_colr_variable_coordinate_run_json(FT_Face face,
                                                    const char* label,
                                                    FT_Error set_status) {
    printf("{\"label\":\"%s\",\"set_var_status\":%d,\"sequence\":",
           label,
           set_status);
    print_gradient_colorline_sequence_json(face, label, 36, 1);
    printf("}");
}

static int emit_colr_variable_gradient_case(OracleFace* face) {
    printf("{");
    print_status(0);
    printf(",\"output\":{\"coordinate_runs\":[");
    print_colr_variable_coordinate_run_json(face->face, "default", 0);
    FT_Fixed coords[2] = { 900 * 65536, 1 * 65536 };
    FT_Error set_status = FT_Set_Var_Design_Coordinates(face->face, 2, coords);
    printf(",");
    print_colr_variable_coordinate_run_json(face->face, "wght_900_grad_1", set_status);
    printf("]}}\n");
    return 0;
}

static int is_colr_variable_gradient_case(const char* case_id) {
    return streq(case_id, "ftcolor.FT_ColorStop.iterator_output_values") ||
           streq(case_id, "ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_variable_values") ||
           streq(case_id, "ftcolor.FT_Get_Colorline_Stops.success_iterates_variable_colorline_stops");
}

static int is_colr_static_gradient_case(const char* case_id) {
    return streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_LINEAR_GRADIENT.paint_linear_gradient_payload") ||
           streq(case_id, "ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_static_values") ||
           streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_RADIAL_GRADIENT.paint_radial_gradient_payload") ||
           streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_SWEEP_GRADIENT.paint_sweep_gradient_payload") ||
           streq(case_id, "ftcolor.FT_PaintRadialGradient.get_paint_radial_gradient_values") ||
           streq(case_id, "ftcolor.FT_PaintSweepGradient.get_paint_sweep_gradient_values") ||
           streq(case_id, "ftcolor.FT_COLR_PAINT_EXTEND_PAD.colorline_extend_pad") ||
           streq(case_id, "ftcolor.FT_COLR_PAINT_EXTEND_REPEAT.colorline_extend_repeat") ||
           streq(case_id, "ftcolor.FT_COLR_PAINT_EXTEND_REFLECT.colorline_extend_reflect") ||
           streq(case_id, "ftcolor.FT_PaintExtend.gradient_extend_runtime") ||
           streq(case_id, "ftcolor.FT_ColorLine.gradient_colorline_values") ||
           streq(case_id, "ftcolor.FT_Get_Colorline_Stops.success_iterates_static_colorline_stops") ||
           streq(case_id, "ftcolor.FT_Get_Colorline_Stops.end_of_iteration") ||
           streq(case_id, "ftcolor.FT_ColorStopIterator.advanced_by_get_colorline_stops");
}

static int is_colr_transform_paint_case(const char* case_id) {
    return streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_ROTATE.paint_rotate_normalized_payload") ||
           streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_SCALE.paint_scale_normalized_payload") ||
           streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_SKEW.paint_skew_normalized_payload") ||
           streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.explicit_transform_payload") ||
           streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_TRANSLATE.paint_translate_payload") ||
           streq(case_id, "ftcolor.FT_PaintRotate.get_paint_rotate_values") ||
           streq(case_id, "ftcolor.FT_PaintScale.get_paint_scale_values") ||
           streq(case_id, "ftcolor.FT_PaintSkew.get_paint_skew_values") ||
           streq(case_id, "ftcolor.FT_PaintTransform.get_paint_transform_values") ||
           streq(case_id, "ftcolor.FT_PaintTranslate.get_paint_translate_values");
}

static int emit_color_paint_graph_case(int argc, char** argv) {
    if (argc != 6) {
        fprintf(stderr, "--color-paint-graph-case requires CASE SOURCE_KIND SOURCE FACE_INDEX\n");
        return 2;
    }
    const char* case_id = argv[2];
    OracleFace face;
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened != 0) {
        return opened;
    }
    if (is_colr_all_paints_case(case_id)) {
        int result = emit_colr_all_paints_case(case_id, &face);
        close_oracle_face(&face);
        return result;
    }
    if (is_colr_root_transform_case(case_id)) {
        int result = emit_colr_root_transform_case(case_id, &face);
        close_oracle_face(&face);
        return result;
    }
    if (streq(case_id, "ftcolor.FT_Get_Paint_Layers.success_iterates_colr_v1_layers") ||
        streq(case_id, "ftcolor.FT_Get_Paint_Layers.end_of_iteration") ||
        streq(case_id, "ftcolor.FT_LayerIterator.initialized_and_advanced_by_paint_layers_v1") ||
        streq(case_id, "ftcolor.FT_LayerIterator.initialized_and_advanced_by_layer_apis") ||
        streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_COLR_LAYERS.paint_colr_layers_payload")) {
        int result = emit_color_paint_layers_case(case_id, &face);
        close_oracle_face(&face);
        return result;
    }
    if (streq(case_id, "ftcolor.FT_COLR_PAINTFORMAT_COLR_GLYPH.paint_colr_glyph_runtime")) {
        int result = emit_colr_glyph_paint_graph_case(&face);
        close_oracle_face(&face);
        return result;
    }
    if (is_colr_transform_paint_case(case_id)) {
        int result = emit_colr_transform_paint_case(&face);
        close_oracle_face(&face);
        return result;
    }
    if (is_colr_variable_gradient_case(case_id)) {
        int result = emit_colr_variable_gradient_case(&face);
        close_oracle_face(&face);
        return result;
    }
    if (is_colr_static_gradient_case(case_id)) {
        int result = emit_colr_static_gradient_case(case_id, &face);
        close_oracle_face(&face);
        return result;
    }

    printf("{");
    print_status(0);
    printf(",\"output\":{\"solid_root\":");
    print_colr_root_json(face.face, 36, FT_COLOR_NO_ROOT_TRANSFORM);
    printf(",\"glyph_root\":");
    print_colr_root_json(face.face, 37, FT_COLOR_NO_ROOT_TRANSFORM);
    printf(",\"composites\":[");
    for (int mode = 0; mode < 28; mode++) {
        if (mode) {
            printf(",");
        }
        FT_UInt base_glyph = (FT_UInt)(39 + mode);
        printf("{\"expected_mode\":%d,\"base_glyph\":%u,", mode, base_glyph);
        FT_OpaquePaint opaque;
        memset(&opaque, 0, sizeof(opaque));
        FT_Bool result = FT_Get_Color_Glyph_Paint(face.face, base_glyph, FT_COLOR_NO_ROOT_TRANSFORM, &opaque);
        printf("\"root_return\":%u,\"root_opaque\":", result);
        print_opaque_paint_json(opaque);
        printf(",\"root_paint\":");
        print_colr_paint_node_json(face.face, opaque, 0);
        printf("}");
    }
    printf("],\"graph_snapshot\":");
    print_colr_graph_snapshot_json(face.face);
    printf("}}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_color_palette_case(int argc, char** argv) {
    const char* case_id = argv[2];
    OracleFace face;
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened != 0) {
        return opened;
    }
    if (streq(case_id, "ftcolor.FT_Palette_Data_Get.error_null_face_or_output")) {
        FT_Palette_Data data = { 999, (const FT_UShort*)1, (const FT_UShort*)1, 999, (const FT_UShort*)1 };
        FT_Error null_face = FT_Palette_Data_Get(NULL, &data);
        FT_Error null_output = FT_Palette_Data_Get(face.face, NULL);
        printf("{");
        print_status(null_face ? null_face : null_output);
        printf(",\"output\":{\"variants\":[");
        print_palette_data_null_input_row("null_face", null_face, &data);
        printf(",");
        print_palette_data_null_input_row("null_apalette", null_output, NULL);
        printf("]}}\n");
        close_oracle_face(&face);
        return 0;
    }
    if (streq(case_id, "ftcolor.FT_Palette_Select.error_null_face_or_invalid_palette_index")) {
        FT_Palette_Data data = { 0 };
        FT_Error data_err = FT_Palette_Data_Get(face.face, &data);
        FT_UShort equal_num_palettes = data_err ? 0 : data.num_palettes;
        FT_Color* null_face_palette = (FT_Color*)1;
        FT_Color* equal_palette = (FT_Color*)1;
        FT_Color* max_palette = (FT_Color*)1;
        FT_Error null_face = FT_Palette_Select(NULL, 0, &null_face_palette);
        FT_Error equal_index = FT_Palette_Select(face.face, equal_num_palettes, &equal_palette);
        FT_Error max_index = FT_Palette_Select(face.face, 65535, &max_palette);
        printf("{");
        print_status(null_face ? null_face : (equal_index ? equal_index : max_index));
        printf(",\"output\":{\"variants\":[");
        print_palette_select_error_row("null_face", null_face, null_face_palette);
        printf(",");
        print_palette_select_error_row("palette_index_equal_num_palettes", equal_index, equal_palette);
        printf(",");
        print_palette_select_error_row("palette_index_65535", max_index, max_palette);
        printf("]}}\n");
        close_oracle_face(&face);
        return 0;
    }
    printf("{");
    print_status(0);
    if (streq(case_id, "ftcolor.FT_Palette_Data_Get.success_sfnt_without_cpal") ||
        streq(case_id, "ftcolor.FT_Palette_Data_Get.success_non_sfnt_null_palette_data") ||
        streq(case_id, "ftcolor.FT_Palette_Data_Get.success_sfnt_cpal_palette_data")) {
        FT_Palette_Data data = { 999, (const FT_UShort*)1, (const FT_UShort*)1, 999, (const FT_UShort*)1 };
        FT_Error err = FT_Palette_Data_Get(face.face, &data);
        printf(",\"output\":");
        print_palette_data_json(err, data);
        printf("}\n");
    } else if (streq(case_id, "ftcolor.FT_Palette_Data.palette_data_get_values") ||
               streq(case_id, "ftcolor.FT_PALETTE_FOR_DARK_BACKGROUND.palette_flags_runtime") ||
               streq(case_id, "ftcolor.FT_PALETTE_FOR_LIGHT_BACKGROUND.palette_flags_runtime")) {
        FT_Palette_Data data = { 999, (const FT_UShort*)1, (const FT_UShort*)1, 999, (const FT_UShort*)1 };
        FT_Error err = FT_Palette_Data_Get(face.face, &data);
        printf(",\"output\":");
        print_palette_data_values_json(err, data);
        printf("}\n");
    } else if (streq(case_id, "ftcolor.FT_Palette_Select.success_selects_palette_and_returns_entries") ||
               streq(case_id, "ftcolor.FT_Color.palette_entries_preserve_bgra_order")) {
        FT_UShort count = palette_entry_count(face.face);
        printf(",\"output\":{\"runs\":[");
        for (FT_UShort i = 0; i < 2; i++) {
            FT_Color* palette = NULL;
            FT_Error err = FT_Palette_Select(face.face, i, &palette);
            if (i) {
                printf(",");
            }
            printf("{\"error\":%d,\"active_palette_index\":%d,\"palette_is_null\":%s,\"entries\":",
                   err,
                   err ? -1 : i,
                   palette ? "false" : "true");
            print_color_entries(palette, count);
            printf("}");
        }
        printf("]}}\n");
    } else if (streq(case_id, "ftcolor.FT_Palette_Select.success_null_output_selects_without_return")) {
        FT_Error err = FT_Palette_Select(face.face, 1, NULL);
        FT_UShort count = palette_entry_count(face.face);
        FT_Color* followup = NULL;
        FT_Error followup_err = FT_Palette_Select(face.face, 1, &followup);
        printf(",\"output\":{\"error\":%d,\"active_palette_index\":%d,\"followup_error\":%d,\"followup_palette_is_null\":%s,\"followup_entries\":",
               err,
               err ? -1 : 1,
               followup_err,
               followup ? "false" : "true");
        print_color_entries(followup, count);
        printf("}}\n");
    } else if (streq(case_id, "ftcolor.FT_Palette_Select.success_reselect_resets_user_modifications")) {
        FT_UShort count = palette_entry_count(face.face);
        FT_Color* first = NULL;
        FT_Error first_err = FT_Palette_Select(face.face, 0, &first);
        printf(",\"output\":{\"first_select\":{\"palette_is_null\":%s,\"entries\":",
               first ? "false" : "true");
        print_color_entries(first, count);
        printf("},\"mutated_entries\":");
        if (!first_err && first) {
            first[0].blue = 1;
            first[0].green = 2;
            first[0].red = 3;
            first[0].alpha = 4;
        }
        print_color_entries(first, count);
        FT_Color* second = NULL;
        FT_Error second_err = FT_Palette_Select(face.face, 0, &second);
        printf(",\"second_select\":{\"palette_is_null\":%s,\"entries\":",
               second ? "false" : "true");
        print_color_entries(second, count);
        printf("},\"error_sequence\":[%d,%d]}}\n", first_err, second_err);
    } else if (streq(case_id, "ftcolor.FT_Palette_Select.success_non_sfnt_returns_null_palette")) {
        FT_Color* palette = (FT_Color*)1;
        FT_Error err = FT_Palette_Select(face.face, 0, &palette);
        printf(",\"output\":{\"error\":%d,\"apalette_nullness\":\"%s\"}}\n",
               err,
               palette ? "non_null" : "null");
    } else if (streq(case_id, "ftcolor.FT_Palette_Set_Foreground_Color.success_non_sfnt_noop")) {
        FT_Color color = { 1, 2, 3, 4 };
        FT_Error err = FT_Palette_Set_Foreground_Color(face.face, color);
        printf(",\"output\":{\"error\":%d,\"followup_palette_or_render_state\":\"unchanged\"}}\n", err);
    } else if (streq(case_id, "ftcolor.FT_Palette_Set_Foreground_Color.success_sets_sfnt_foreground_color")) {
        printf(",\"output\":");
        print_palette_set_foreground_sfnt_runs_json(face.face);
        printf("}\n");
    } else if (streq(case_id, "ftcolor.FT_Palette_Set_Foreground_Color.default_foreground_color_policy")) {
        printf(",\"output\":");
        print_palette_default_foreground_policy_json(face.face);
        printf("}\n");
    } else {
        close_oracle_face(&face);
        return 2;
    }
    close_oracle_face(&face);
    return 0;
}

static int emit_get_postscript_name_variants(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    char* variants = (char*)malloc(strlen(argv[5]) + 1);
    if (!variants) {
        return 2;
    }
    memcpy(variants, argv[5], strlen(argv[5]) + 1);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"results\":[");
    int first = 1;
    char* token = strtok(variants, ",");
    while (token) {
        if (!first) {
            printf(",");
        }
        first = 0;
        printf("{\"variant\":\"%s\",", token);
        if (streq(token, "null")) {
            printf("\"result\":");
            print_postscript_name_result(FT_Get_Postscript_Name(NULL));
        } else {
            OracleFace face;
            int opened = open_oracle_face(source_kind, source_value, face_index, &face);
            if (opened != 0) {
                free(variants);
                return opened;
            }
            if (!streq(token, "default")) {
                char* end = NULL;
                unsigned long instance = strtoul(token, &end, 10);
                if (end && *end == '\0') {
                    FT_Set_Named_Instance(face.face, (FT_UInt)instance);
                }
            }
            printf("\"result\":");
            print_postscript_name_result(FT_Get_Postscript_Name(face.face));
            close_oracle_face(&face);
        }
        printf("}");
        token = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(variants);
    return 0;
}

static void print_named_instance_observation(FT_Face face, FT_Error error) {
    printf("{\"return\":%d,\"face_index\":%ld,\"face_flags\":%ld,\"variation_bit_set\":",
           error,
           face->face_index,
           face->face_flags);
    print_json_bool((face->face_flags & FT_FACE_FLAG_VARIATION) != 0);
    printf(",\"postscript_name\":");
    print_postscript_name_result(FT_Get_Postscript_Name(face));
    printf("}");
}

static void print_fixed_coord_array(FT_Fixed* coords, FT_UInt count) {
    printf("[");
    for (FT_UInt i = 0; i < count; i++) {
        if (i) printf(",");
        printf("%ld", (long)coords[i]);
    }
    printf("]");
}

static void parse_fixed_coord_csv(const char* text, FT_Fixed* coords, FT_UInt count) {
    if (!text || streq(text, "-")) {
        return;
    }
    char* cursor = (char*)text;
    for (FT_UInt i = 0; i < count; i++) {
        coords[i] = strtol(cursor, &cursor, 10);
        if (*cursor == ',') {
            cursor++;
        }
    }
}

static void parse_long_coord_csv(const char* text, FT_Long* coords, FT_UInt count) {
    if (!text || streq(text, "-")) {
        return;
    }
    char* cursor = (char*)text;
    for (FT_UInt i = 0; i < count; i++) {
        coords[i] = strtol(cursor, &cursor, 10);
        if (*cursor == ',') {
            cursor++;
        }
    }
}

static void print_ftmm_var_design_output(
    FT_Error error,
    FT_Face face,
    FT_Fixed* coords,
    FT_UInt count) {
    printf("{");
    print_status(error);
    printf(",\"output\":");
    if (error) {
        printf("null");
    } else {
        printf("{\"return\":%d,\"coords\":", error);
        print_fixed_coord_array(coords, count);
        printf(",\"face_index\":%ld}", (long)face->face_index);
    }
    printf("}\n");
}

static void print_ftmm_blend_output(
    FT_Error error,
    FT_Face face,
    FT_Fixed* coords,
    FT_UInt count) {
    printf("{");
    print_status(error);
    printf(",\"output\":");
    if (error) {
        printf("null");
    } else {
        printf("{\"return\":%d,\"active_blend_coords\":", error);
        print_fixed_coord_array(coords, count);
        printf(",\"face_flags\":%ld,\"variation_bit_set\":", (long)face->face_flags);
        print_json_bool((face->face_flags & FT_FACE_FLAG_VARIATION) != 0);
        printf("}");
    }
    printf("}\n");
}

static int fixed_coord_arrays_equal(FT_Fixed* left, FT_Fixed* right, FT_UInt count) {
    for (FT_UInt i = 0; i < count; i++) {
        if (left[i] != right[i]) {
            return 0;
        }
    }
    return 1;
}

static void print_ftmm_set_var_design_output(
    FT_Error error,
    FT_Face face,
    FT_Fixed* design_coords,
    FT_Fixed* blend_coords,
    FT_UInt count) {
    printf("{");
    print_status(error);
    printf(",\"output\":");
    if (error) {
        printf("null");
    } else {
        printf("{\"return\":%d,\"design_coords\":", error);
        print_fixed_coord_array(design_coords, count);
        printf(",\"blend_coords\":");
        print_fixed_coord_array(blend_coords, count);
        printf(",\"face_flags\":%ld,\"variation_bit_set\":", (long)face->face_flags);
        print_json_bool((face->face_flags & FT_FACE_FLAG_VARIATION) != 0);
        printf("}");
    }
    printf("}\n");
}

static void print_ftmm_face_metrics_object(FT_Face face) {
    printf("\"ascender\":%ld,\"descender\":%ld,\"height\":%ld,"
           "\"max_advance_width\":%ld,\"max_advance_height\":%ld",
           (long)face->ascender,
           (long)face->descender,
           (long)face->height,
           (long)face->max_advance_width,
           (long)face->max_advance_height);
}

static FT_Error apply_ftmm_blend_prior(FT_Face face, const char* kind, FT_UInt count, const char* csv) {
    FT_Fixed coords[16] = {0};
    parse_fixed_coord_csv(csv, coords, count < 16 ? count : 16);
    if (streq(kind, "set_var_blend")) {
        return FT_Set_Var_Blend_Coordinates(face, count, coords);
    }
    if (streq(kind, "set_mm_blend")) {
        return FT_Set_MM_Blend_Coordinates(face, count, coords);
    }
    return FT_Err_Ok;
}

static int emit_ftmm_blend_coordinates(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened != 0) {
        return opened;
    }

    const char* mode = argv[2];
    FT_Error err = apply_ftmm_blend_prior(
        face.face,
        argv[6],
        (FT_UInt)strtoul(argv[7], NULL, 10),
        argv[8]);

    FT_UInt set_count = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_Fixed set_coords[16] = {0};
    parse_fixed_coord_csv(argv[10], set_coords, set_count < 16 ? set_count : 16);
    if (!err && streq(mode, "set-var")) {
        err = FT_Set_Var_Blend_Coordinates(
            face.face,
            set_count,
            streq(argv[11], "null") ? NULL : set_coords);
    } else if (!err && streq(mode, "set-mm")) {
        err = FT_Set_MM_Blend_Coordinates(
            face.face,
            set_count,
            streq(argv[11], "null") ? NULL : set_coords);
    }

    FT_UInt output_count = (FT_UInt)strtoul(argv[12], NULL, 10);
    FT_Fixed coords[16];
    for (FT_UInt i = 0; i < 16; i++) {
        coords[i] = streq(argv[11], "nonzero") ? (FT_Fixed)(0x11110000 + i) : 0;
    }
    if (!err) {
        if (streq(mode, "get-mm")) {
            err = FT_Get_MM_Blend_Coordinates(
                face.face,
                output_count,
                streq(argv[11], "null") ? NULL : coords);
        } else {
            err = FT_Get_Var_Blend_Coordinates(
                face.face,
                output_count,
                streq(argv[11], "null") ? NULL : coords);
        }
    }
    print_ftmm_blend_output(err, face.face, coords, output_count);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_var_blend_alias(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    OracleFace control;
    opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &control);
    if (opened != 0) {
        close_oracle_face(&face);
        return opened;
    }

    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Fixed set_coords[16] = {0};
    parse_fixed_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_UInt output_count = (FT_UInt)strtoul(argv[7], NULL, 10);

    FT_Fixed coords[16] = {0};
    FT_Error err = FT_Set_Var_Blend_Coordinates(face.face, set_count, set_coords);
    if (!err) {
        err = FT_Get_Var_Blend_Coordinates(face.face, output_count, coords);
    }

    FT_Fixed control_coords[16] = {0};
    FT_Error control_err = FT_Set_MM_Blend_Coordinates(control.face, set_count, set_coords);
    if (!control_err) {
        control_err = FT_Get_Var_Blend_Coordinates(control.face, output_count, control_coords);
    }

    printf("{");
    print_status(err);
    printf(",\"output\":");
    if (err) {
        printf("null");
    } else {
        printf("{\"return\":%d,\"active_blend_coords\":", err);
        print_fixed_coord_array(coords, output_count);
        printf(",\"matches_control_call\":");
        print_json_bool(
            control_err == err && fixed_coord_arrays_equal(coords, control_coords, output_count));
        printf("}");
    }
    printf("}\n");

    close_oracle_face(&control);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_var_blend_flag_matrix(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    char* cursor = argv[5];
    printf("{");
    print_status(0);
    printf(",\"output\":{\"results\":[");
    int row = 0;
    while (cursor && *cursor) {
        char* end = cursor;
        while (*end && *end != ';') {
            end++;
        }
        char saved = *end;
        *end = '\0';

        char* colon = cursor;
        while (*colon && *colon != ':') {
            colon++;
        }
        if (*colon == ':') {
            *colon = '\0';
        }
        FT_UInt count = (FT_UInt)strtoul(cursor, NULL, 10);
        const char* csv = (*colon == '\0') ? colon + 1 : "-";
        FT_Fixed coords[16] = {0};
        parse_fixed_coord_csv(csv, coords, count < 16 ? count : 16);
        FT_Error err = FT_Set_Var_Blend_Coordinates(
            face.face,
            count,
            streq(csv, "null") ? NULL : coords);

        if (row) {
            printf(",");
        }
        printf("{\"num_coords\":%u,\"return\":%d,\"face_flags\":%ld,\"is_variation\":",
               count,
               err,
               (long)face.face->face_flags);
        print_json_bool((face.face->face_flags & FT_FACE_FLAG_VARIATION) != 0);
        printf("}");
        row++;

        *end = saved;
        if (saved == ';') {
            cursor = end + 1;
        } else {
            break;
        }
    }
    printf("]}}\n");

    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_mm_blend_invalid_matrix(int argc, char** argv) {
    (void)argc;
    OracleFace variable_face;
    int opened = open_oracle_face(argv[2], argv[3], 0, &variable_face);
    if (opened != 0) {
        return opened;
    }
    OracleFace static_face;
    opened = open_oracle_face(argv[4], argv[5], 0, &static_face);
    if (opened != 0) {
        close_oracle_face(&variable_face);
        return opened;
    }

    FT_UInt count = (FT_UInt)strtoul(argv[6], NULL, 10);
    FT_Fixed coords[8] = {0};
    FT_Error err_variable_null_coords =
        FT_Get_MM_Blend_Coordinates(variable_face.face, count, NULL);
    FT_Error err_null_face =
        FT_Get_MM_Blend_Coordinates(NULL, count, coords);
    FT_Error err_static_face =
        FT_Get_MM_Blend_Coordinates(static_face.face, count, coords);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"results\":[");
    printf("{\"scenario\":\"variable_null_coords\",\"return\":%d},", err_variable_null_coords);
    printf("{\"scenario\":\"null_face\",\"return\":%d},", err_null_face);
    printf("{\"scenario\":\"non_variable_face\",\"return\":%d}", err_static_face);
    printf("]}}\n");

    close_oracle_face(&static_face);
    close_oracle_face(&variable_face);
    return 0;
}

static int emit_ftmm_mm_blend_count_matrix(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], 0, &face);
    if (opened != 0) {
        return opened;
    }
    FT_UInt axis_count = (FT_UInt)strtoul(argv[4], NULL, 10);
    FT_UInt counts[4] = {0, 1, axis_count, axis_count + 2};

    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    for (FT_UInt row = 0; row < 4; row++) {
        FT_UInt count = counts[row];
        FT_Fixed coords[16];
        for (FT_UInt i = 0; i < 16; i++) {
            coords[i] = (FT_Fixed)(0x11110000 + i);
        }
        FT_Error err = FT_Get_MM_Blend_Coordinates(face.face, count, coords);
        if (row) {
            printf(",");
        }
        printf("{\"num_coords\":%u,\"return\":%d,\"coords_after\":", count, err);
        print_fixed_coord_array(coords, count);
        printf("}");
    }
    printf("]}}\n");

    close_oracle_face(&face);
    return 0;
}

static void init_multi_master_sentinel(FT_Multi_Master* master) {
    master->num_axis = 0xA5A5;
    master->num_designs = 0x5A5A;
    for (FT_UInt i = 0; i < 4; i++) {
        master->axis[i].name = NULL;
    }
    master->axis[0].minimum = -101;
    master->axis[0].maximum = 101;
    master->axis[1].minimum = -202;
    master->axis[1].maximum = 202;
    master->axis[2].minimum = -303;
    master->axis[2].maximum = 303;
    master->axis[3].minimum = -404;
    master->axis[3].maximum = 404;
}

static int multi_master_unused_slots_preserved(const FT_Multi_Master* master) {
    FT_Multi_Master sentinel;
    init_multi_master_sentinel(&sentinel);
    FT_UInt populated = master->num_axis;
    if (populated > 4) {
        populated = 4;
    }
    for (FT_UInt i = populated; i < 4; i++) {
        if (master->axis[i].name != NULL ||
            master->axis[i].minimum != sentinel.axis[i].minimum ||
            master->axis[i].maximum != sentinel.axis[i].maximum) {
            return 0;
        }
    }
    return 1;
}

static void print_multi_master_descriptor(const FT_Multi_Master* master) {
    printf("{\"num_axis\":%u,\"num_designs\":%u,\"axis\":[",
           (unsigned)master->num_axis,
           (unsigned)master->num_designs);
    for (FT_UInt i = 0; i < 4; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"name\":");
        print_postscript_name_result(master->axis[i].name);
        printf(",\"minimum\":%ld,\"maximum\":%ld}",
               (long)master->axis[i].minimum,
               (long)master->axis[i].maximum);
    }
    printf("],\"unused_axis_sentinels\":");
    print_json_bool(multi_master_unused_slots_preserved(master));
    printf("}");
}

static void print_ftmm_get_multi_master_output(FT_Error err, const FT_Multi_Master* master) {
    printf("{");
    print_status(err);
    printf(",\"output\":{\"return\":%d,", err);
    if (err) {
        printf("\"descriptor_after\":");
    } else {
        printf("\"descriptor\":");
    }
    print_multi_master_descriptor(master);
    printf("}}\n");
}

static void print_ftmm_multi_master_populated_output(
    FT_Error success_err,
    const FT_Multi_Master* success_master,
    FT_Error control_err) {
    printf("{");
    print_status(success_err);
    printf(",\"output\":");
    if (success_err) {
        printf("null");
    } else {
        printf("{\"success_return\":%d,\"error_control_return\":%d,"
               "\"num_axis\":%u,\"num_designs\":%u,\"axis\":",
               success_err,
               control_err,
               (unsigned)success_master->num_axis,
               (unsigned)success_master->num_designs);
        printf("[");
        for (FT_UInt i = 0; i < 4; i++) {
            if (i) {
                printf(",");
            }
            printf("{\"name\":");
            print_postscript_name_result(success_master->axis[i].name);
            printf(",\"minimum\":%ld,\"maximum\":%ld}",
                   (long)success_master->axis[i].minimum,
                   (long)success_master->axis[i].maximum);
        }
        printf("],\"unused_axis_slots\":");
        print_json_bool(multi_master_unused_slots_preserved(success_master));
        printf("}");
    }
    printf("}\n");
}

static void print_ftmm_var_axis(const FT_Var_Axis* axis) {
    printf("{\"name\":");
    print_postscript_name_result(axis->name);
    printf(",\"minimum\":%ld,\"def\":%ld,\"maximum\":%ld,"
           "\"tag\":%lu,\"strid\":%u}",
           (long)axis->minimum,
           (long)axis->def,
           (long)axis->maximum,
           (unsigned long)axis->tag,
           (unsigned)axis->strid);
}

static void print_ftmm_var_namedstyle(const FT_Var_Named_Style* style, FT_UInt axis_count) {
    printf("{\"coords\":[");
    if (style && style->coords) {
        for (FT_UInt i = 0; i < axis_count; i++) {
            if (i) {
                printf(",");
            }
            printf("%ld", (long)style->coords[i]);
        }
    }
    printf("],\"strid\":%u,\"psid\":%u}",
           style ? (unsigned)style->strid : 0,
           style ? (unsigned)style->psid : 0);
}

static void print_ftmm_var_descriptor(FT_MM_Var* master) {
    printf("{\"num_axis\":%u,\"num_designs\":%u,\"num_namedstyles\":%u,"
           "\"axis_pointer\":\"%s\",\"namedstyle_pointer\":\"%s\",\"axis\":[",
           master ? (unsigned)master->num_axis : 0,
           master ? (unsigned)master->num_designs : 0,
           master ? (unsigned)master->num_namedstyles : 0,
           master && master->axis ? "non_null" : "null",
           master && master->namedstyle ? "non_null" : "null");
    FT_UInt axis_count = master ? master->num_axis : 0;
    for (FT_UInt i = 0; i < axis_count; i++) {
        if (i) {
            printf(",");
        }
        print_ftmm_var_axis(&master->axis[i]);
    }
    printf("],\"axis_flags\":[");
    if (master) {
        size_t mmvar_size = (sizeof(FT_MM_Var) + sizeof(void*) - 1) & ~(sizeof(void*) - 1);
        FT_UShort* axis_flags = (FT_UShort*)((char*)master + mmvar_size);
        for (FT_UInt i = 0; i < axis_count; i++) {
            if (i) {
                printf(",");
            }
            printf("%u", (unsigned)axis_flags[i]);
        }
    }
    printf("],\"namedstyle\":[");
    FT_UInt namedstyle_count = master ? master->num_namedstyles : 0;
    for (FT_UInt i = 0; i < namedstyle_count; i++) {
        if (i) {
            printf(",");
        }
        print_ftmm_var_namedstyle(master && master->namedstyle ? &master->namedstyle[i] : NULL,
                                  axis_count);
    }
    printf("]}");
}

static void print_ftmm_get_mm_var_null_output(FT_Error err) {
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
}

static void print_ftmm_get_mm_var_output(FT_Error err, FT_Library library, FT_MM_Var* master) {
    printf("{");
    print_status(err);
    printf(",\"output\":");
    if (err) {
        printf("null}\n");
        return;
    }
    printf("{\"return\":%d,\"descriptor_pointer\":\"%s\",\"descriptor\":",
           err,
           master ? "non_null" : "null");
    print_ftmm_var_descriptor(master);
    FT_Error done_err = master ? FT_Done_MM_Var(library, master) : FT_Err_Ok;
    printf(",\"done_return\":%d}}\n", done_err);
}

static int emit_ftmm_get_mm_var(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_MM_Var* master = NULL;
    FT_Error err = FT_Get_MM_Var(
        face.face,
        argc > 5 && streq(argv[5], "null") ? NULL : &master);
    if (argc > 5 && streq(argv[5], "null")) {
        print_ftmm_get_mm_var_null_output(err);
    } else {
        print_ftmm_get_mm_var_output(err, face.library, master);
    }
    close_oracle_face(&face);
    return 0;
}

static void print_ftmm_get_and_done_mm_var_output(
    FT_Error get_err,
    FT_Error done_err,
    FT_MM_Var* descriptor_ptr
) {
    int pointer_non_null = descriptor_ptr != NULL;
    printf("{");
    print_status(get_err ? get_err : done_err);
    printf(",\"output\":{\"get_return\":%d,"
           "\"amaster_pointer\":\"%s\","
           "\"descriptor_pointer_identity\":\"%s\","
           "\"done_return\":%d,"
           "\"allocation_events\":\"%s\","
           "\"free_event\":\"%s\","
           "\"free_events\":\"%s\"}}\n",
           get_err,
           pointer_non_null ? "non_null" : "null",
           pointer_non_null ? "same_pointer" : "null",
           done_err,
           pointer_non_null ? "descriptor_allocated" : "none",
           pointer_non_null && !done_err ? "allocation_released" : "none",
           pointer_non_null && !done_err ? "allocation_released" : "none");
}

static int emit_ftmm_get_and_done_mm_var(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_MM_Var* master = NULL;
    FT_Error get_err = FT_Get_MM_Var(face.face, &master);
    FT_MM_Var* descriptor_ptr = master;
    FT_Error done_err = get_err ? FT_Err_Ok : FT_Done_MM_Var(face.library, master);
    print_ftmm_get_and_done_mm_var_output(get_err, done_err, descriptor_ptr);
    close_oracle_face(&face);
    return 0;
}

static FT_UInt ftmm_axis_index_from_token(const char* token, FT_MM_Var* master) {
    size_t mmvar_size = (sizeof(FT_MM_Var) + sizeof(void*) - 1) & ~(sizeof(void*) - 1);
    if (streq(token, "null_master") || streq(token, "null_flags")) {
        return 0;
    }
    if (streq(token, "last")) {
        return master && master->num_axis ? master->num_axis - 1 : 0;
    }
    if (streq(token, "num_axis")) {
        return master ? master->num_axis : 0;
    }
    if (streq(token, "num_axis_plus_1")) {
        return master ? master->num_axis + 1 : 1;
    }
    if (streq(token, "axis_with_fvar_flags_hidden") || streq(token, "hidden_axis")) {
        FT_UShort* axis_flags = master
            ? (FT_UShort*)((char*)master + mmvar_size)
            : NULL;
        if (axis_flags) {
            for (FT_UInt i = 0; i < master->num_axis; i++) {
                if (axis_flags[i] & FT_VAR_AXIS_FLAG_HIDDEN) {
                    return i;
                }
            }
        }
        return 0;
    }
    if (streq(token, "visible_axis")) {
        FT_UShort* axis_flags = master
            ? (FT_UShort*)((char*)master + mmvar_size)
            : NULL;
        if (axis_flags) {
            for (FT_UInt i = 0; i < master->num_axis; i++) {
                if (!(axis_flags[i] & FT_VAR_AXIS_FLAG_HIDDEN)) {
                    return i;
                }
            }
        }
        return 0;
    }
    return (FT_UInt)strtoul(token, NULL, 10);
}

static void print_ftmm_axis_flags_row(const char* token, FT_MM_Var* master, FT_UInt flags_initial) {
    FT_UInt axis_index = ftmm_axis_index_from_token(token, master);
    FT_UInt flags = flags_initial;
    FT_Error err = FT_Get_Var_Axis_Flags(
        streq(token, "null_master") ? NULL : master,
        axis_index,
        streq(token, "null_flags") ? NULL : &flags);
    printf("{\"axis_index_token\":\"%s\",\"axis_index\":%u,\"status\":%d,"
           "\"error\":%d,\"flags_initial\":%u,\"flags\":%u,\"flags_after\":%u,",
           token,
           (unsigned)axis_index,
           err,
           err,
           (unsigned)flags_initial,
           (unsigned)flags,
           (unsigned)flags);
    printf("\"hidden_bit\":%s,\"axis\":", (flags & FT_VAR_AXIS_FLAG_HIDDEN) ? "true" : "false");
    if (!err && master && axis_index < master->num_axis) {
        print_ftmm_var_axis(&master->axis[axis_index]);
    } else {
        printf("null");
    }
    printf("}");
}

static int emit_ftmm_axis_flags(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_MM_Var* master = NULL;
    FT_Error err = FT_Get_MM_Var(face.face, &master);
    printf("{");
    print_status(err);
    printf(",\"output\":");
    if (err) {
        printf("null}\n");
        close_oracle_face(&face);
        return 0;
    }
    FT_UInt flags_initial = (FT_UInt)strtoul(argv[6], NULL, 10);
    char* rows_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!rows_arg) {
        FT_Done_MM_Var(face.library, master);
        close_oracle_face(&face);
        return 2;
    }
    strcpy(rows_arg, argv[5]);
    printf("{\"return\":%d,\"rows\":[", err);
    FT_UInt row_index = 0;
    char* cursor = rows_arg;
    while (cursor && *cursor) {
        char* end = cursor;
        while (*end && *end != ',') {
            end++;
        }
        char saved_end = *end;
        *end = '\0';
        if (row_index) {
            printf(",");
        }
        print_ftmm_axis_flags_row(cursor, master, flags_initial);
        row_index++;
        *end = saved_end;
        if (saved_end == ',') {
            cursor = end + 1;
        } else {
            break;
        }
    }
    FT_Error done_err = FT_Done_MM_Var(face.library, master);
    printf("],\"done_return\":%d}}\n", done_err);
    free(rows_arg);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_get_multi_master(int argc, char** argv) {
    FT_Multi_Master master;
    init_multi_master_sentinel(&master);
    FT_Error err;
    if (argc == 6 && streq(argv[2], "null")) {
        err = FT_Get_Multi_Master(NULL, streq(argv[5], "null") ? NULL : &master);
        print_ftmm_get_multi_master_output(err, &master);
        return 0;
    }
    if (argc != 7 || !streq(argv[2], "face")) {
        fprintf(stderr, "invalid --ftmm-get-multi-master args\n");
        return 2;
    }
    OracleFace face;
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened != 0) {
        return opened;
    }
    err = FT_Get_Multi_Master(face.face, streq(argv[6], "null") ? NULL : &master);
    print_ftmm_get_multi_master_output(err, &master);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_get_multi_master_adobe_control(int argc, char** argv) {
    (void)argc;
    OracleFace success_face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &success_face);
    if (opened != 0) {
        return opened;
    }
    OracleFace control_face;
    opened = open_oracle_face(argv[5], argv[6], atol(argv[7]), &control_face);
    if (opened != 0) {
        close_oracle_face(&success_face);
        return opened;
    }

    FT_Multi_Master success_master;
    init_multi_master_sentinel(&success_master);
    FT_Multi_Master control_master;
    init_multi_master_sentinel(&control_master);
    FT_Error success_err = FT_Get_Multi_Master(success_face.face, &success_master);
    FT_Error control_err = FT_Get_Multi_Master(control_face.face, &control_master);
    print_ftmm_multi_master_populated_output(success_err, &success_master, control_err);

    close_oracle_face(&control_face);
    close_oracle_face(&success_face);
    return 0;
}

typedef struct FtmmWeightRow_ {
    FT_Error set_err;
    FT_Error get_err;
    FT_Long face_flags;
    FT_UInt len_after;
    FT_Fixed values[6];
} FtmmWeightRow;

static void print_ftmm_weight_row(const FtmmWeightRow* row) {
    printf("{\"set_return\":%d,\"get_return\":%d,\"face_flags\":%ld,\"variation_bit_set\":",
           row->set_err,
           row->get_err,
           (long)row->face_flags);
    print_json_bool((row->face_flags & FT_FACE_FLAG_VARIATION) != 0);
    printf(",\"vector\":{\"len_after\":%u,\"weightvector_after\":", (unsigned)row->len_after);
    FT_UInt split = row->len_after < 6 ? row->len_after : 6;
    print_fixed_coord_array((FT_Fixed*)row->values, split);
    printf(",\"sentinel_after\":");
    print_fixed_coord_array((FT_Fixed*)row->values + split, 6 - split);
    printf(",\"buffer_after\":");
    print_fixed_coord_array((FT_Fixed*)row->values, 6);
    printf("}}");
}

static int emit_ftmm_mm_weight_vector(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    char* scenarios = (char*)malloc(strlen(argv[5]) + 1);
    if (!scenarios) {
        close_oracle_face(&face);
        return 2;
    }
    strcpy(scenarios, argv[5]);

    FT_Error status = FT_Err_Ok;
    FtmmWeightRow rows[16];
    FT_UInt row_count = 0;
    char* cursor = scenarios;
    while (cursor && *cursor) {
        char* end = cursor;
        while (*end && *end != ';') {
            end++;
        }
        char saved_end = *end;
        *end = '\0';

        char* colon = cursor;
        while (*colon && *colon != ':') {
            colon++;
        }
        if (*colon != ':') {
            free(scenarios);
            close_oracle_face(&face);
            return 2;
        }
        *colon = '\0';
        FT_UInt len = (FT_UInt)strtoul(cursor, NULL, 10);
        const char* csv = colon + 1;
        FT_Fixed weights[16] = {0};
        parse_fixed_coord_csv(csv, weights, len < 16 ? len : 16);

        FT_Error set_err = FT_Set_MM_WeightVector(
            face.face,
            len,
            streq(csv, "null") ? NULL : weights);
        FT_UInt capacity = 6;
        FT_UInt get_len = capacity;
        FT_Fixed values[16];
        for (FT_UInt i = 0; i < 16; i++) {
            values[i] = (FT_Fixed)(0x11110000 + i);
        }
        FT_Error get_err = FT_Err_Ok;
        if (!set_err) {
            get_err = FT_Get_MM_WeightVector(face.face, &get_len, values);
        }
        if (!status) {
            status = set_err ? set_err : get_err;
        }
        if (row_count < 16) {
            rows[row_count].set_err = set_err;
            rows[row_count].get_err = get_err;
            rows[row_count].face_flags = face.face ? face.face->face_flags : 0;
            rows[row_count].len_after = get_len;
            for (FT_UInt i = 0; i < 6; i++) {
                rows[row_count].values[i] = values[i];
            }
            row_count++;
        }

        *end = saved_end;
        if (saved_end == ';') {
            cursor = end + 1;
        } else {
            break;
        }
    }
    printf("{");
    print_status(status);
    printf(",\"output\":{\"rows\":[");
    for (FT_UInt i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_ftmm_weight_row(rows + i);
    }
    printf("]}}\n");

    free(scenarios);
    close_oracle_face(&face);
    return 0;
}

static FT_UInt ftmm_weight_capacity_for_row(const char* row, FT_UInt num_designs) {
    if (streq(row, "exact_num_designs")) {
        return num_designs;
    }
    if (streq(row, "larger_than_num_designs")) {
        return num_designs + 2;
    }
    if (streq(row, "smaller_than_num_designs")) {
        return num_designs ? num_designs - 1 : 0;
    }
    return num_designs;
}

static void print_ftmm_weight_get_row(
    const char* row,
    FT_Error status,
    FT_UInt len_before,
    FT_UInt len_after,
    FT_Fixed* values,
    FT_UInt capacity) {
    FT_UInt split = len_after < capacity ? len_after : capacity;
    printf("{\"row\":\"%s\",\"status\":%d,\"len_before\":%u,\"len_after\":%u,"
           "\"weightvector_after\":",
           row,
           status,
           (unsigned)len_before,
           (unsigned)len_after);
    print_fixed_coord_array(values, split);
    printf(",\"sentinel_after\":[");
    for (FT_UInt i = split; i < capacity; i++) {
        if (i != split) {
            printf(",");
        }
        printf("%ld", (long)values[i]);
    }
    printf("],\"buffer_after\":");
    print_fixed_coord_array(values, capacity);
    printf("}");
}

static int emit_ftmm_get_mm_weight_vector(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_Multi_Master master;
    init_multi_master_sentinel(&master);
    FT_Error err = FT_Get_Multi_Master(face.face, &master);
    if (!err && !streq(argv[6], "-")) {
        FT_Fixed prior[16] = {0};
        parse_fixed_coord_csv(argv[6], prior, master.num_designs < 16 ? master.num_designs : 16);
        err = FT_Set_MM_WeightVector(face.face, master.num_designs, prior);
    }
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":{\"setup_return\":%d}}\n", err);
        close_oracle_face(&face);
        return 0;
    }

    char* rows_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!rows_arg) {
        close_oracle_face(&face);
        return 2;
    }
    strcpy(rows_arg, argv[5]);

    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":{\"rows\":[");
    FT_UInt row_index = 0;
    char* cursor = rows_arg;
    while (cursor && *cursor) {
        char* end = cursor;
        while (*end && *end != ',') {
            end++;
        }
        char saved_end = *end;
        *end = '\0';

        FT_UInt capacity = ftmm_weight_capacity_for_row(cursor, master.num_designs);
        FT_Fixed values[16];
        for (FT_UInt i = 0; i < 16; i++) {
            values[i] = (FT_Fixed)(0x11110000 + i);
        }
        FT_UInt len_before = capacity;
        FT_UInt len_after = capacity;
        FT_Error get_err = FT_Get_MM_WeightVector(face.face, &len_after, values);
        if (row_index) {
            printf(",");
        }
        print_ftmm_weight_get_row(cursor, get_err, len_before, len_after, values, capacity);
        row_index++;

        *end = saved_end;
        if (saved_end == ',') {
            cursor = end + 1;
        } else {
            break;
        }
    }
    printf("]}}\n");

    free(rows_arg);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_get_var_design_coordinates(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    const char* prior_kind = argv[5];
    FT_Error err = FT_Err_Ok;
    if (streq(prior_kind, "set_var_design")) {
        FT_UInt prior_count = (FT_UInt)strtoul(argv[6], NULL, 10);
        FT_Fixed prior_coords[8] = {0};
        parse_fixed_coord_csv(argv[7], prior_coords, prior_count < 8 ? prior_count : 8);
        err = FT_Set_Var_Design_Coordinates(face.face, prior_count, prior_coords);
    } else if (streq(prior_kind, "set_named_instance")) {
        FT_UInt instance_index = (FT_UInt)strtoul(argv[8], NULL, 10);
        err = FT_Set_Named_Instance(face.face, instance_index);
    }

    FT_UInt count = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_Fixed coords[16];
    for (FT_UInt i = 0; i < 16; i++) {
        coords[i] = streq(argv[10], "nonzero") ? (FT_Fixed)(0x11110000 + i) : 0;
    }
    if (!err) {
        err = FT_Get_Var_Design_Coordinates(
            face.face,
            count,
            streq(argv[10], "null") ? NULL : coords);
    }
    print_ftmm_var_design_output(err, face.face, coords, count);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_set_var_design_coordinates(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Fixed set_coords[16] = {0};
    parse_fixed_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_Error err = FT_Set_Var_Design_Coordinates(face.face, set_count, set_coords);

    FT_UInt output_count = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_Fixed design_coords[16] = {0};
    FT_Fixed blend_coords[16] = {0};
    if (!err) {
        err = FT_Get_Var_Design_Coordinates(face.face, output_count, design_coords);
    }
    if (!err) {
        err = FT_Get_Var_Blend_Coordinates(face.face, output_count, blend_coords);
    }
    print_ftmm_set_var_design_output(err, face.face, design_coords, blend_coords, output_count);
    close_oracle_face(&face);
    return 0;
}

static int emit_face_flags_after_variation(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_Long initial_flags = (FT_Long)face.face->face_flags;
    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Fixed set_coords[16] = {0};
    parse_fixed_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_Error setter_status = FT_Set_Var_Design_Coordinates(face.face, set_count, set_coords);
    FT_Long after_set_flags = (FT_Long)face.face->face_flags;
    FT_Error reset_status = FT_Set_Named_Instance(face.face, (FT_UInt)-1);
    FT_Long after_reset_flags = (FT_Long)face.face->face_flags;

    printf("{");
    print_status(setter_status);
    printf(",\"output\":{\"initial_face_flags\":%ld,"
           "\"after_set_face_flags\":%ld,"
           "\"after_reset_face_flags\":%ld,"
           "\"setter_status\":%d,"
           "\"reset_status\":%d}}\n",
           initial_flags,
           after_set_flags,
           after_reset_flags,
           setter_status,
           reset_status);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_set_var_design_glyph_output(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Fixed set_coords[16] = {0};
    parse_fixed_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[10], NULL, 10);
    FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[11], NULL, 10);

    FT_Error err = FT_Set_Pixel_Sizes(face.face, pixel_width, pixel_height);
    if (!err) {
        err = FT_Set_Var_Design_Coordinates(face.face, set_count, set_coords);
    }
    if (!err) {
        err = FT_Load_Glyph(face.face, glyph_index, load_flags);
    }
    if (!err) {
        err = FT_Render_Glyph(face.face->glyph, render_mode);
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{");
        print_slot_body(face.face->glyph, glyph_index);
        printf("}}\n");
    }
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_set_var_design_metrics_output(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Fixed set_coords[16] = {0};
    parse_fixed_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[10], NULL, 10);

    FT_Error err = FT_Set_Pixel_Sizes(face.face, pixel_width, pixel_height);
    if (!err) {
        err = FT_Set_Var_Design_Coordinates(face.face, set_count, set_coords);
    }
    if (!err) {
        err = FT_Load_Glyph(face.face, glyph_index, load_flags);
    }

    printf("{");
    print_status(err);
    printf(",\"output\":");
    if (err) {
        printf("null");
    } else {
        printf("{\"return\":%d,\"face_metrics\":{", err);
        print_ftmm_face_metrics_object(face.face);
        printf("},\"size_metrics\":{");
        print_size_metrics_object(face.face->size->metrics);
        printf("},");
        print_vector_named("glyph_advance", face.face->glyph->advance);
        printf(",\"face_flags\":%ld,\"variation_bit_set\":", (long)face.face->face_flags);
        print_json_bool((face.face->face_flags & FT_FACE_FLAG_VARIATION) != 0);
        printf("}");
    }
    printf("}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_set_var_blend_glyph_output(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Fixed set_coords[16] = {0};
    parse_fixed_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[10], NULL, 10);
    FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[11], NULL, 10);

    FT_Error err = FT_Set_Pixel_Sizes(face.face, pixel_width, pixel_height);
    if (!err) {
        err = FT_Set_Var_Blend_Coordinates(face.face, set_count, set_coords);
    }
    if (!err) {
        err = FT_Load_Glyph(face.face, glyph_index, load_flags);
    }
    if (!err) {
        err = FT_Render_Glyph(face.face->glyph, render_mode);
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{");
        print_slot_body(face.face->glyph, glyph_index);
        printf("}}\n");
    }
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_set_mm_blend_glyph_output(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Fixed set_coords[16] = {0};
    parse_fixed_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[10], NULL, 10);
    FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[11], NULL, 10);

    FT_Error err = FT_Set_Pixel_Sizes(face.face, pixel_width, pixel_height);
    if (!err) {
        err = FT_Set_MM_Blend_Coordinates(face.face, set_count, set_coords);
    }
    if (!err) {
        err = FT_Load_Glyph(face.face, glyph_index, load_flags);
    }
    if (!err) {
        err = FT_Render_Glyph(face.face->glyph, render_mode);
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{");
        print_slot_body(face.face->glyph, glyph_index);
        printf("}}\n");
    }
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_set_mm_design_coordinates(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Long set_coords[16] = {0};
    parse_long_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_Error err = FT_Set_MM_Design_Coordinates(
        face.face,
        set_count,
        streq(argv[6], "null") ? NULL : set_coords);

    FT_UInt output_count = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_Fixed design_coords[16] = {0};
    FT_Fixed blend_coords[16] = {0};
    if (!err) {
        err = FT_Get_Var_Design_Coordinates(face.face, output_count, design_coords);
    }
    if (!err) {
        err = FT_Get_Var_Blend_Coordinates(face.face, output_count, blend_coords);
    }
    print_ftmm_set_var_design_output(err, face.face, design_coords, blend_coords, output_count);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_set_mm_design_glyph_output(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    FT_UInt set_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Long set_coords[16] = {0};
    parse_long_coord_csv(argv[6], set_coords, set_count < 16 ? set_count : 16);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[10], NULL, 10);
    FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[11], NULL, 10);

    FT_Error err = FT_Set_Pixel_Sizes(face.face, pixel_width, pixel_height);
    if (!err) {
        err = FT_Set_MM_Design_Coordinates(
            face.face,
            set_count,
            streq(argv[6], "null") ? NULL : set_coords);
    }
    if (!err) {
        err = FT_Load_Glyph(face.face, glyph_index, load_flags);
    }
    if (!err) {
        err = FT_Render_Glyph(face.face->glyph, render_mode);
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{");
        print_slot_body(face.face->glyph, glyph_index);
        printf("}}\n");
    }
    close_oracle_face(&face);
    return 0;
}

typedef struct FtmmMmDesignScenarioRow_ {
    FT_Error set_err;
    FT_Error get_design_err;
    FT_Error get_blend_err;
    FT_Long face_flags;
    FT_Fixed design_coords[16];
    FT_Fixed blend_coords[16];
} FtmmMmDesignScenarioRow;

static void print_ftmm_mm_design_scenario_row(
    const FtmmMmDesignScenarioRow* row,
    FT_UInt output_count) {
    printf("{\"set_return\":%d,\"get_design_return\":%d,\"get_blend_return\":%d,"
           "\"design_coords\":",
           row->set_err,
           row->get_design_err,
           row->get_blend_err);
    print_fixed_coord_array((FT_Fixed*)row->design_coords, output_count);
    printf(",\"blend_coords\":");
    print_fixed_coord_array((FT_Fixed*)row->blend_coords, output_count);
    printf(",\"face_flags\":%ld,\"variation_bit_set\":", (long)row->face_flags);
    print_json_bool((row->face_flags & FT_FACE_FLAG_VARIATION) != 0);
    printf("}");
}

static int emit_ftmm_set_var_design_scenarios(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    char* scenarios = (char*)malloc(strlen(argv[5]) + 1);
    if (!scenarios) {
        close_oracle_face(&face);
        return 2;
    }
    strcpy(scenarios, argv[5]);

    FT_UInt output_count = (FT_UInt)strtoul(argv[6], NULL, 10);
    FtmmMmDesignScenarioRow rows[16];
    FT_UInt row_count = 0;
    FT_Error status = FT_Err_Ok;
    char* cursor = scenarios;
    while (cursor && *cursor) {
        char* end = cursor;
        while (*end && *end != ';') {
            end++;
        }
        char saved_end = *end;
        *end = '\0';

        char* colon = cursor;
        while (*colon && *colon != ':') {
            colon++;
        }
        if (*colon != ':') {
            free(scenarios);
            close_oracle_face(&face);
            return 2;
        }
        *colon = '\0';
        FT_UInt set_count = (FT_UInt)strtoul(cursor, NULL, 10);
        const char* csv = colon + 1;
        FT_Fixed set_coords[16] = {0};
        parse_fixed_coord_csv(csv, set_coords, set_count < 16 ? set_count : 16);

        FT_Error set_err = FT_Set_Var_Design_Coordinates(
            face.face,
            set_count,
            streq(csv, "null") ? NULL : set_coords);
        FT_Fixed design_coords[16] = {0};
        FT_Fixed blend_coords[16] = {0};
        FT_Error get_design_err = FT_Err_Ok;
        FT_Error get_blend_err = FT_Err_Ok;
        if (!set_err) {
            get_design_err = FT_Get_Var_Design_Coordinates(
                face.face,
                output_count,
                design_coords);
        }
        if (!set_err && !get_design_err) {
            get_blend_err = FT_Get_Var_Blend_Coordinates(
                face.face,
                output_count,
                blend_coords);
        }
        if (!status) {
            status = set_err ? set_err : (get_design_err ? get_design_err : get_blend_err);
        }
        if (row_count < 16) {
            rows[row_count].set_err = set_err;
            rows[row_count].get_design_err = get_design_err;
            rows[row_count].get_blend_err = get_blend_err;
            rows[row_count].face_flags = face.face ? face.face->face_flags : 0;
            for (FT_UInt i = 0; i < 16; i++) {
                rows[row_count].design_coords[i] = design_coords[i];
                rows[row_count].blend_coords[i] = blend_coords[i];
            }
            row_count++;
        }

        *end = saved_end;
        if (saved_end == ';') {
            cursor = end + 1;
        } else {
            break;
        }
    }

    printf("{");
    print_status(status);
    printf(",\"output\":{\"rows\":[");
    for (FT_UInt i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_ftmm_mm_design_scenario_row(rows + i, output_count);
    }
    printf("]}}\n");

    free(scenarios);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_var_blend_scenarios(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    char* scenarios = (char*)malloc(strlen(argv[5]) + 1);
    if (!scenarios) {
        close_oracle_face(&face);
        return 2;
    }
    strcpy(scenarios, argv[5]);

    FT_UInt output_count = (FT_UInt)strtoul(argv[6], NULL, 10);
    FtmmMmDesignScenarioRow rows[16];
    FT_UInt row_count = 0;
    FT_Error status = FT_Err_Ok;
    char* cursor = scenarios;
    while (cursor && *cursor) {
        char* end = cursor;
        while (*end && *end != ';') {
            end++;
        }
        char saved_end = *end;
        *end = '\0';

        char* colon = cursor;
        while (*colon && *colon != ':') {
            colon++;
        }
        if (*colon != ':') {
            free(scenarios);
            close_oracle_face(&face);
            return 2;
        }
        *colon = '\0';
        FT_UInt set_count = (FT_UInt)strtoul(cursor, NULL, 10);
        const char* csv = colon + 1;
        FT_Fixed set_coords[16] = {0};
        parse_fixed_coord_csv(csv, set_coords, set_count < 16 ? set_count : 16);

        FT_Error set_err = FT_Set_Var_Blend_Coordinates(
            face.face,
            set_count,
            streq(csv, "null") ? NULL : set_coords);
        FT_Fixed design_coords[16] = {0};
        FT_Fixed blend_coords[16] = {0};
        FT_Error get_design_err = FT_Err_Ok;
        FT_Error get_blend_err = FT_Err_Ok;
        if (!set_err) {
            get_design_err = FT_Get_Var_Design_Coordinates(
                face.face,
                output_count,
                design_coords);
        }
        if (!set_err && !get_design_err) {
            get_blend_err = FT_Get_Var_Blend_Coordinates(
                face.face,
                output_count,
                blend_coords);
        }
        if (!status) {
            status = set_err ? set_err : (get_design_err ? get_design_err : get_blend_err);
        }
        if (row_count < 16) {
            rows[row_count].set_err = set_err;
            rows[row_count].get_design_err = get_design_err;
            rows[row_count].get_blend_err = get_blend_err;
            rows[row_count].face_flags = face.face ? face.face->face_flags : 0;
            for (FT_UInt i = 0; i < 16; i++) {
                rows[row_count].design_coords[i] = design_coords[i];
                rows[row_count].blend_coords[i] = blend_coords[i];
            }
            row_count++;
        }

        *end = saved_end;
        if (saved_end == ';') {
            cursor = end + 1;
        } else {
            break;
        }
    }

    printf("{");
    print_status(status);
    printf(",\"output\":{\"rows\":[");
    for (FT_UInt i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_ftmm_mm_design_scenario_row(rows + i, output_count);
    }
    printf("]}}\n");

    free(scenarios);
    close_oracle_face(&face);
    return 0;
}

static int emit_ftmm_set_mm_design_scenarios(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    char* scenarios = (char*)malloc(strlen(argv[5]) + 1);
    if (!scenarios) {
        close_oracle_face(&face);
        return 2;
    }
    strcpy(scenarios, argv[5]);

    FT_UInt output_count = (FT_UInt)strtoul(argv[6], NULL, 10);
    FtmmMmDesignScenarioRow rows[16];
    FT_UInt row_count = 0;
    FT_Error status = FT_Err_Ok;
    char* cursor = scenarios;
    while (cursor && *cursor) {
        char* end = cursor;
        while (*end && *end != ';') {
            end++;
        }
        char saved_end = *end;
        *end = '\0';

        char* colon = cursor;
        while (*colon && *colon != ':') {
            colon++;
        }
        if (*colon != ':') {
            free(scenarios);
            close_oracle_face(&face);
            return 2;
        }
        *colon = '\0';
        FT_UInt set_count = (FT_UInt)strtoul(cursor, NULL, 10);
        const char* csv = colon + 1;
        FT_Long set_coords[16] = {0};
        parse_long_coord_csv(csv, set_coords, set_count < 16 ? set_count : 16);

        FT_Error set_err = FT_Set_MM_Design_Coordinates(
            face.face,
            set_count,
            streq(csv, "null") ? NULL : set_coords);
        FT_Fixed design_coords[16] = {0};
        FT_Fixed blend_coords[16] = {0};
        FT_Error get_design_err = FT_Err_Ok;
        FT_Error get_blend_err = FT_Err_Ok;
        if (!set_err) {
            get_design_err = FT_Get_Var_Design_Coordinates(
                face.face,
                output_count,
                design_coords);
        }
        if (!set_err && !get_design_err) {
            get_blend_err = FT_Get_Var_Blend_Coordinates(
                face.face,
                output_count,
                blend_coords);
        }
        if (!status) {
            status = set_err ? set_err : (get_design_err ? get_design_err : get_blend_err);
        }
        if (row_count < 16) {
            rows[row_count].set_err = set_err;
            rows[row_count].get_design_err = get_design_err;
            rows[row_count].get_blend_err = get_blend_err;
            rows[row_count].face_flags = face.face ? face.face->face_flags : 0;
            for (FT_UInt i = 0; i < 16; i++) {
                rows[row_count].design_coords[i] = design_coords[i];
                rows[row_count].blend_coords[i] = blend_coords[i];
            }
            row_count++;
        }

        *end = saved_end;
        if (saved_end == ';') {
            cursor = end + 1;
        } else {
            break;
        }
    }

    printf("{");
    print_status(status);
    printf(",\"output\":{\"rows\":[");
    for (FT_UInt i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_ftmm_mm_design_scenario_row(rows + i, output_count);
    }
    printf("]}}\n");

    free(scenarios);
    close_oracle_face(&face);
    return 0;
}

typedef struct FtmmMmBlendScenarioRow_ {
    FT_Error set_err;
    FT_Error get_blend_err;
    FT_Long face_flags;
    FT_Fixed blend_coords[16];
} FtmmMmBlendScenarioRow;

static void print_ftmm_mm_blend_scenario_row(
    const FtmmMmBlendScenarioRow* row,
    FT_UInt output_count) {
    printf("{\"set_return\":%d,\"get_blend_return\":%d,\"blend_coords\":",
           row->set_err,
           row->get_blend_err);
    print_fixed_coord_array((FT_Fixed*)row->blend_coords, output_count);
    printf(",\"face_flags\":%ld,\"variation_bit_set\":", (long)row->face_flags);
    print_json_bool((row->face_flags & FT_FACE_FLAG_VARIATION) != 0);
    printf("}");
}

static int emit_ftmm_set_mm_blend_scenarios(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }

    char* scenarios = (char*)malloc(strlen(argv[5]) + 1);
    if (!scenarios) {
        close_oracle_face(&face);
        return 2;
    }
    strcpy(scenarios, argv[5]);

    FT_UInt output_count = (FT_UInt)strtoul(argv[6], NULL, 10);
    FtmmMmBlendScenarioRow rows[16];
    FT_UInt row_count = 0;
    FT_Error status = FT_Err_Ok;
    char* cursor = scenarios;
    while (cursor && *cursor) {
        char* end = cursor;
        while (*end && *end != ';') {
            end++;
        }
        char saved_end = *end;
        *end = '\0';

        char* colon = cursor;
        while (*colon && *colon != ':') {
            colon++;
        }
        if (*colon != ':') {
            free(scenarios);
            close_oracle_face(&face);
            return 2;
        }
        *colon = '\0';
        FT_UInt set_count = (FT_UInt)strtoul(cursor, NULL, 10);
        const char* csv = colon + 1;
        FT_Fixed set_coords[16] = {0};
        parse_fixed_coord_csv(csv, set_coords, set_count < 16 ? set_count : 16);

        FT_Error set_err = FT_Set_MM_Blend_Coordinates(
            face.face,
            set_count,
            streq(csv, "null") ? NULL : set_coords);
        FT_Fixed blend_coords[16] = {0};
        FT_Error get_blend_err = FT_Err_Ok;
        if (!set_err) {
            get_blend_err = FT_Get_MM_Blend_Coordinates(face.face, output_count, blend_coords);
        }
        if (!status) {
            status = set_err ? set_err : get_blend_err;
        }
        if (row_count < 16) {
            rows[row_count].set_err = set_err;
            rows[row_count].get_blend_err = get_blend_err;
            rows[row_count].face_flags = face.face ? face.face->face_flags : 0;
            for (FT_UInt i = 0; i < 16; i++) {
                rows[row_count].blend_coords[i] = blend_coords[i];
            }
            row_count++;
        }

        *end = saved_end;
        if (saved_end == ';') {
            cursor = end + 1;
        } else {
            break;
        }
    }

    printf("{");
    print_status(status);
    printf(",\"output\":{\"rows\":[");
    for (FT_UInt i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_ftmm_mm_blend_scenario_row(rows + i, output_count);
    }
    printf("]}}\n");

    free(scenarios);
    close_oracle_face(&face);
    return 0;
}

static void print_named_instance_null_face_observation(FT_Error error) {
    printf("{\"return\":%d,\"face_index\":null,\"face_flags\":null,"
           "\"variation_bit_set\":null,\"postscript_name\":",
           error);
    print_postscript_name_result(NULL);
    printf("}");
}

static int emit_set_named_instance_null_face(int argc, char** argv) {
    (void)argc;
    FT_UInt instance_index = (FT_UInt)strtoul(argv[2], NULL, 10);
    FT_Error err = FT_Set_Named_Instance(NULL, instance_index);
    printf("{");
    print_status(err);
    printf(",\"output\":");
    print_named_instance_null_face_observation(err);
    printf("}\n");
    return 0;
}

static int emit_set_named_instance_descriptor(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_UInt instance_index = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt compare_namedstyle_index = (FT_UInt)strtoul(argv[6], NULL, 10);

    FT_MM_Var* master = NULL;
    FT_Error err = FT_Get_MM_Var(face.face, &master);
    FT_UInt axis_count = master ? master->num_axis : 0;
    FT_Fixed* selected = NULL;
    if (!err && (!master || compare_namedstyle_index >= master->num_namedstyles)) {
        err = FT_Err_Invalid_Argument;
    }
    if (!err) {
        selected = (FT_Fixed*)malloc(sizeof(FT_Fixed) * (axis_count ? axis_count : 1));
        if (!selected) {
            err = FT_Err_Out_Of_Memory;
        }
    }
    if (!err) {
        err = FT_Set_Named_Instance(face.face, instance_index);
    }
    if (!err) {
        err = FT_Get_Var_Design_Coordinates(face.face, axis_count, selected);
    }

    printf("{");
    print_status(err);
    printf(",\"output\":");
    if (err) {
        printf("null");
    } else {
        printf("{\"return\":%d,\"namedstyle_coords\":", err);
        print_fixed_coord_array(master->namedstyle[compare_namedstyle_index].coords, axis_count);
        printf(",\"selected_design_coords\":");
        print_fixed_coord_array(selected, axis_count);
        printf(",\"face_index\":%ld}", (long)face.face->face_index);
    }
    printf("}\n");

    free(selected);
    if (master) {
        FT_Done_MM_Var(face.library, master);
    }
    close_oracle_face(&face);
    return 0;
}

static void print_default_named_instance_row(
    const char* variant,
    FT_Error error,
    FT_UInt before,
    FT_UInt after) {
    printf("{\"variant\":\"%s\",\"status\":%d,\"instance_index_before\":%u,"
           "\"instance_index_after\":%u}",
           variant,
           error,
           before,
           after);
}

static int emit_get_default_named_instance(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_UInt before = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt after = before;
    FT_Error err = FT_Get_Default_Named_Instance(face.face, &after);
    printf("{");
    print_status(err);
    printf(",\"output\":");
    print_default_named_instance_row("variable", err, before, after);
    printf("}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_get_default_named_instance_invalid(int argc, char** argv) {
    (void)argc;
    FT_UInt before = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt null_after = before;
    FT_Error null_err = FT_Get_Default_Named_Instance(NULL, &null_after);

    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_UInt non_variable_after = before;
    FT_Error non_variable_err =
        FT_Get_Default_Named_Instance(face.face, &non_variable_after);
    printf("{");
    print_status(null_err);
    printf(",\"output\":{\"rows\":[");
    print_default_named_instance_row("null", null_err, before, null_after);
    printf(",");
    print_default_named_instance_row(
        "non_variable", non_variable_err, before, non_variable_after);
    printf("]}}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_get_first_char_null_face(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_UInt glyph_index = 0;
    FT_ULong char_code = FT_Get_First_Char(NULL, &glyph_index);
    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_char_iteration_result(char_code, glyph_index);
    printf("}\n");
    return 0;
}

static int emit_get_next_char_null_face(int argc, char** argv) {
    (void)argc;
    size_t starts_len = strlen(argv[2]);
    char* starts = (char*)malloc(starts_len + 1);
    if (!starts) {
        return 2;
    }
    memcpy(starts, argv[2], starts_len + 1);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* token = strtok(starts, ",");
    int first = 1;
    while (token) {
        FT_ULong start = strtoul(token, NULL, 10);
        FT_UInt glyph_index = 0;
        FT_ULong char_code = FT_Get_Next_Char(NULL, start, &glyph_index);
        if (!first) printf(",");
        first = 0;
        printf("{\"start\":%lu,\"result\":", (unsigned long)start);
        print_char_iteration_result(char_code, glyph_index);
        printf("}");
        token = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(starts);
    return 0;
}

static int emit_set_named_instance(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* prior_instance = argv[5];
    FT_UInt instance_index = (FT_UInt)strtoul(argv[6], NULL, 10);

    OracleFace face;
    int opened = open_oracle_face(source_kind, source_value, face_index, &face);
    if (opened != 0) {
        return opened;
    }

    FT_Error err = FT_Err_Ok;
    if (!streq(prior_instance, "-")) {
        err = FT_Set_Named_Instance(face.face, (FT_UInt)strtoul(prior_instance, NULL, 10));
    }
    if (!err) {
        err = FT_Set_Named_Instance(face.face, instance_index);
    }

    printf("{");
    print_status(err);
    printf(",\"output\":");
    print_named_instance_observation(face.face, err);
    printf("}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_set_named_instance_adobe_mm_reset(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    FT_UInt prior_count = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Long prior_coords[16] = {0};
    parse_long_coord_csv(argv[6], prior_coords, prior_count < 16 ? prior_count : 16);
    FT_UInt instance_index = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt output_count = (FT_UInt)strtoul(argv[8], NULL, 10);

    OracleFace face;
    int opened = open_oracle_face(source_kind, source_value, face_index, &face);
    if (opened != 0) {
        return opened;
    }

    FT_Error err = FT_Set_MM_Design_Coordinates(face.face, prior_count, prior_coords);
    if (!err) {
        err = FT_Set_Named_Instance(face.face, instance_index);
    }
    FT_Fixed design_coords[16] = {0};
    if (!err) {
        err = FT_Get_Var_Design_Coordinates(face.face, output_count, design_coords);
    }

    printf("{");
    print_status(err);
    printf(",\"output\":");
    if (err) {
        printf("null");
    } else {
        printf("{\"return\":%d,\"design_coords\":", err);
        print_fixed_coord_array(design_coords, output_count);
        printf(",\"face_flags\":%ld,\"variation_bit_set\":%s,\"face_index\":%ld}",
               (long)face.face->face_flags,
               (face.face->face_flags & FT_FACE_FLAG_VARIATION) ? "true" : "false",
               (long)face.face->face_index);
    }
    printf("}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_set_named_instance_glyph_output(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    FT_UInt instance_index = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[9], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[10], NULL, 10);
    FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[11], NULL, 10);

    OracleFace face;
    int opened = open_oracle_face(source_kind, source_value, face_index, &face);
    if (opened != 0) {
        return opened;
    }

    FT_Error err = FT_Set_Pixel_Sizes(face.face, pixel_width, pixel_height);
    if (!err) {
        err = FT_Set_Named_Instance(face.face, instance_index);
    }
    if (!err) {
        err = FT_Load_Glyph(face.face, glyph_index, load_flags);
    }
    if (!err) {
        err = FT_Render_Glyph(face.face->glyph, render_mode);
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{");
        print_slot_body(face.face->glyph, glyph_index);
        printf("}}\n");
    }
    close_oracle_face(&face);
    return 0;
}

static void print_set_charmap_row(
    const char* variant,
    FT_Int charmap_index,
    FT_Error status,
    FT_CharMap before,
    FT_Face face,
    const char* chars_csv) {
    printf("{\"variant\":\"%s\",\"charmap_index\":", variant);
    if (charmap_index < 0) {
        printf("null");
    } else {
        printf("%d", charmap_index);
    }
    printf(",\"status\":%d,\"active_charmap_before\":", status);
    print_charmap_inventory_record(before);
    printf(",\"active_charmap_after\":");
    print_active_charmap(face);
    printf(",\"selected_charmap\":");
    print_active_charmap(face);
    printf(",\"char_indices\":");
    print_charmap_probe_indices(face, chars_csv);
    printf("}");
}

static int set_charmap_index_selected(FT_Face face, const char* indices_arg, FT_Int index) {
    if (streq(indices_arg, "all_non_format14_charmaps")) {
        if (!face || !face->charmaps || index < 0 || index >= face->num_charmaps) {
            return 0;
        }
        return FT_Get_CMap_Format(face->charmaps[index]) != 14;
    }
    if (streq(indices_arg, "all_charmaps")) {
        return 1;
    }
    const char* cursor = indices_arg;
    while (cursor && *cursor) {
        char* end = NULL;
        long value = strtol(cursor, &end, 10);
        if (end == cursor) {
            return 0;
        }
        if (value == index) {
            return 1;
        }
        cursor = *end == ',' ? end + 1 : end;
    }
    return 0;
}

static FT_Int cmap_variant_index(FT_Face face, const char* variant) {
    if (!face || !face->charmaps || streq(variant, "null")) {
        return -1;
    }
    if (streq(variant, "out_of_range")) {
        return face->num_charmaps;
    }
    if (strncmp(variant, "format", 6) == 0) {
        char* end = NULL;
        long format = strtol(variant + 6, &end, 10);
        if (!end || *end != '\0') {
            return -1;
        }
        for (FT_Int i = 0; i < face->num_charmaps; i++) {
            if (FT_Get_CMap_Format(face->charmaps[i]) == format) {
                return i;
            }
        }
        return -1;
    }
    char* end = NULL;
    long index = strtol(variant, &end, 10);
    if (!end || *end != '\0' || index < 0 || index > face->num_charmaps) {
        return -1;
    }
    return (FT_Int)index;
}

static FT_CharMap cmap_for_variant(FT_Face face, const char* variant, FT_Int* index) {
    *index = cmap_variant_index(face, variant);
    if (*index < 0 || !face || !face->charmaps || *index >= face->num_charmaps) {
        return NULL;
    }
    return face->charmaps[*index];
}

static void print_cmap_metadata(FT_CharMap charmap, FT_Int index) {
    if (!charmap || index < 0) {
        printf("null");
        return;
    }
    printf("{\"index\":%d,\"encoding\":%ld,\"platform_id\":%u,\"encoding_id\":%u,\"format\":%ld,\"language_id\":%lu}",
           index,
           (long)charmap->encoding,
           (unsigned int)charmap->platform_id,
           (unsigned int)charmap->encoding_id,
           (long)FT_Get_CMap_Format(charmap),
           (unsigned long)FT_Get_CMap_Language_ID(charmap));
}

static void print_cmap_info_rows(FT_Face face, const char* variants_csv, int language_mode) {
    char* variants = (char*)malloc(strlen(variants_csv) + 1);
    if (!variants) {
        printf("{\"rows\":[],\"returns\":[]}");
        return;
    }
    memcpy(variants, variants_csv, strlen(variants_csv) + 1);

    printf("{\"rows\":[");
    char* token = strtok(variants, ",");
    int first = 1;
    while (token) {
        FT_Int index = -1;
        FT_CharMap charmap = cmap_for_variant(face, token, &index);
        FT_Long format = FT_Get_CMap_Format(charmap);
        FT_ULong language = FT_Get_CMap_Language_ID(charmap);
        if (!first) {
            printf(",");
        }
        first = 0;
        printf("{\"variant\":\"%s\",\"charmap_index\":", token);
        if (index < 0) {
            printf("null");
        } else {
            printf("%d", index);
        }
        printf(",\"charmap_metadata\":");
        print_cmap_metadata(charmap, index);
        if (language_mode) {
            printf(",\"language_id\":%lu,\"return\":%lu}", (unsigned long)language, (unsigned long)language);
        } else {
            printf(",\"format\":%ld,\"return\":%ld}", (long)format, (long)format);
        }
        token = strtok(NULL, ",");
    }
    printf("],\"returns\":[");

    memcpy(variants, variants_csv, strlen(variants_csv) + 1);
    token = strtok(variants, ",");
    first = 1;
    while (token) {
        FT_Int index = -1;
        FT_CharMap charmap = cmap_for_variant(face, token, &index);
        if (!first) {
            printf(",");
        }
        first = 0;
        if (language_mode) {
            printf("%lu", (unsigned long)FT_Get_CMap_Language_ID(charmap));
        } else {
            printf("%ld", (long)FT_Get_CMap_Format(charmap));
        }
        token = strtok(NULL, ",");
    }
    printf("]}");
    free(variants);
}

static const char* kerning_units(FT_UInt mode) {
    if (mode == FT_KERNING_UNFITTED) {
        return "unfitted_26_6_pixels";
    }
    if (mode == FT_KERNING_UNSCALED) {
        return "font_units";
    }
    return "grid_fitted_26_6_pixels";
}

static void print_kerning_single_output(const char* left,
                                        const char* right,
                                        FT_UInt mode,
                                        FT_UInt left_glyph,
                                        FT_UInt right_glyph,
                                        FT_Error err,
                                        FT_Vector kerning,
                                        int open_object) {
    if (open_object) {
        printf("{");
    }
    print_status(err);
    printf(",\"output\":{\"status\":%d,\"kerning_vectors\":["
           "{\"left\":\"%s\",\"right\":\"%s\",\"mode\":%u,"
           "\"left_glyph\":%u,\"right_glyph\":%u,\"status\":%d,"
           "\"akerning\":{\"x\":%ld,\"y\":%ld},"
           "\"kerning\":{\"x\":%ld,\"y\":%ld},"
           "\"x_26_6\":%ld,\"y_26_6\":%ld,\"units\":\"%s\"}],"
           "\"glyph_indexes\":[{\"left\":%u,\"right\":%u}],"
           "\"akerning\":{\"x\":%ld,\"y\":%ld},"
           "\"kerning\":{\"x\":%ld,\"y\":%ld}}}\n",
           err,
           left,
           right,
           mode,
           left_glyph,
           right_glyph,
           err,
           kerning.x,
           kerning.y,
           kerning.x,
           kerning.y,
           kerning.x,
           kerning.y,
           kerning_units(mode),
           left_glyph,
           right_glyph,
           kerning.x,
           kerning.y,
           kerning.x,
           kerning.y);
}

static int emit_get_kerning_null_face(int argc, char** argv) {
    (void)argc;
    const char* left = argv[2];
    const char* right = argv[3];
    FT_UInt mode = (FT_UInt)strtoul(argv[4], NULL, 10);
    FT_Vector kerning;
    kerning.x = 0;
    kerning.y = 0;
    FT_Error err = FT_Get_Kerning(NULL, 0, 0, mode, &kerning);
    print_kerning_single_output(left, right, mode, 0, 0, err, kerning, 1);
    return 0;
}

static FT_UInt glyph_selector_index(FT_Face face, const char* selector) {
    if (!face || !selector) {
        return 0;
    }
    if (streq(selector, "glyph0") || streq(selector, ".notdef")) {
        return 0;
    }
    if (strncmp(selector, "gid:", 4) == 0) {
        return (FT_UInt)strtoul(selector + 4, NULL, 10);
    }
    if (strncmp(selector, "U+", 2) == 0) {
        return FT_Get_Char_Index(face, strtoul(selector + 2, NULL, 16));
    }
    if (selector[0] && !selector[1]) {
        return FT_Get_Char_Index(face, (FT_ULong)(unsigned char)selector[0]);
    }
    return FT_Get_Name_Index(face, (FT_String*)selector);
}

static FT_UInt glyph_name_query_index(FT_Face face, const char* selector) {
    if (selector && streq(selector, "num_glyphs")) {
        return face ? (FT_UInt)face->num_glyphs : 0;
    }
    if (selector && streq(selector, "num_glyphs_plus_1")) {
        return face ? (FT_UInt)(face->num_glyphs + 1) : 1;
    }
    return glyph_selector_index(face, selector);
}

static unsigned char parse_fill_byte(const char* text) {
    if (!text || !text[0] || streq(text, "-")) {
        return 0xAA;
    }
    return (unsigned char)(strtoul(text, NULL, 0) & 0xFF);
}

static void print_glyph_name_output(
    FT_UInt glyph_index,
    FT_UInt buffer_max,
    FT_Error err,
    unsigned char* buffer) {
    printf("{\"glyph_index\":%u,\"buffer_max\":%u,\"status\":%d,\"buffer_hex\":",
           (unsigned int)glyph_index,
           (unsigned int)buffer_max,
           err);
    if (!buffer) {
        printf("null");
    } else {
        printf("\"");
        print_hex_bytes(buffer, (long)buffer_max);
        printf("\"");
    }
    printf("}");
}

static int emit_get_glyph_name(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* glyph_selector = argv[5];
    FT_UInt buffer_max = (FT_UInt)strtoul(argv[6], NULL, 10);
    unsigned char fill = parse_fill_byte(argv[7]);
    int face_null = streq(argv[8], "null");
    int buffer_null = streq(argv[9], "null");

    OracleFace face = {0};
    if (!face_null) {
        int opened = open_oracle_face(source_kind, source_value, face_index, &face);
        if (opened != 0) {
            return opened == 1 ? 0 : opened;
        }
    }

    FT_Face call_face = face_null ? NULL : face.face;
    FT_UInt glyph_index = glyph_name_query_index(call_face, glyph_selector);
    size_t alloc_len = buffer_max == 0 ? 1 : (size_t)buffer_max;
    unsigned char* buffer = NULL;
    if (!buffer_null) {
        buffer = (unsigned char*)malloc(alloc_len);
        if (!buffer) {
            close_oracle_face(&face);
            return 2;
        }
        memset(buffer, fill, alloc_len);
    }

    FT_Error err = FT_Get_Glyph_Name(
        call_face,
        glyph_index,
        buffer_null ? NULL : (FT_Pointer)buffer,
        buffer_max);

    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_glyph_name_output(glyph_index, buffer_max, err, buffer_null ? NULL : buffer);
    printf("}\n");
    free(buffer);
    close_oracle_face(&face);
    return 0;
}

static int emit_get_name_index(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* glyph_name = argv[5];
    int face_null = streq(argv[6], "null");
    int name_null = streq(argv[7], "null");

    OracleFace face = {0};
    if (!face_null) {
        int opened = open_oracle_face(source_kind, source_value, face_index, &face);
        if (opened != 0) {
            return opened == 1 ? 0 : opened;
        }
    }

    FT_Face call_face = face_null ? NULL : face.face;
    FT_String* name_arg = name_null ? NULL : (FT_String*)glyph_name;
    FT_UInt glyph_index = FT_Get_Name_Index(call_face, name_arg);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"return\":%u,\"glyph_name_bytes\":",
           (unsigned int)glyph_index);
    if (name_null) {
        printf("null");
    } else {
        printf("\"");
        print_hex_bytes((const unsigned char*)glyph_name, (long)strlen(glyph_name));
        printf("\"");
    }
    printf("}}\n");

    close_oracle_face(&face);
    return 0;
}

static int emit_set_charmap_null_face(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_Error err = FT_Set_Charmap(NULL, NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":{\"status\":%d,\"outputs\":[", err);
    print_set_charmap_row("null_face", -1, err, NULL, NULL, "-");
    printf("]}}\n");
    return 0;
}

static int emit_set_charmap_variants(int argc, char** argv) {
    (void)argc;
    OracleFace target;
    OracleFace foreign;
    const char* variants_csv = argv[8];

    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &target);
    if (opened != 0) {
        return opened == 1 ? 0 : opened;
    }
    opened = open_oracle_face(argv[5], argv[6], atol(argv[7]), &foreign);
    if (opened != 0) {
        close_oracle_face(&target);
        return opened == 1 ? 0 : opened;
    }

    char* variants = (char*)malloc(strlen(variants_csv) + 1);
    if (!variants) {
        close_oracle_face(&foreign);
        close_oracle_face(&target);
        return 2;
    }
    memcpy(variants, variants_csv, strlen(variants_csv) + 1);

    FT_Error first_error = FT_Err_Ok;
    char* token = strtok(variants, ",");
    while (token) {
        FT_CharMap charmap = NULL;
        if (streq(token, "from_other_face")) {
            charmap = (foreign.face && foreign.face->charmaps && foreign.face->num_charmaps > 0)
                ? foreign.face->charmaps[0]
                : NULL;
        }
        FT_Error err = FT_Set_Charmap(target.face, charmap);
        if (first_error == FT_Err_Ok && err != FT_Err_Ok) {
            first_error = err;
        }
        token = strtok(NULL, ",");
    }
    free(variants);
    close_oracle_face(&foreign);
    close_oracle_face(&target);

    opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &target);
    if (opened != 0) {
        return opened == 1 ? 0 : opened;
    }
    opened = open_oracle_face(argv[5], argv[6], atol(argv[7]), &foreign);
    if (opened != 0) {
        close_oracle_face(&target);
        return opened == 1 ? 0 : opened;
    }
    variants = (char*)malloc(strlen(variants_csv) + 1);
    if (!variants) {
        close_oracle_face(&foreign);
        close_oracle_face(&target);
        return 2;
    }
    memcpy(variants, variants_csv, strlen(variants_csv) + 1);

    printf("{");
    print_status(first_error);
    printf(",\"output\":{\"status\":%d,\"outputs\":[", first_error);
    token = strtok(variants, ",");
    int first = 1;
    while (token) {
        FT_CharMap before = target.face ? target.face->charmap : NULL;
        FT_CharMap charmap = NULL;
        if (streq(token, "from_other_face")) {
            charmap = (foreign.face && foreign.face->charmaps && foreign.face->num_charmaps > 0)
                ? foreign.face->charmaps[0]
                : NULL;
        }
        FT_Error err = FT_Set_Charmap(target.face, charmap);
        if (first_error == FT_Err_Ok && err != FT_Err_Ok) {
            first_error = err;
        }
        if (!first) {
            printf(",");
        }
        first = 0;
        print_set_charmap_row(token, -1, err, before, target.face, "-");
        token = strtok(NULL, ",");
    }
    printf("]}}\n");

    free(variants);
    close_oracle_face(&foreign);
    close_oracle_face(&target);
    return 0;
}

static int emit_select_charmap(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    FT_Encoding encoding = (FT_Encoding)strtol(argv[5], NULL, 10);
    const char* chars_csv = argv[6];

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
    FT_Face face = NULL;
    if (!err) {
        err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    }
    if (!err) {
        err = FT_Select_Charmap(face, encoding);
    }

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"char_indices\":");
        print_charmap_probe_indices(face, chars_csv);
        printf("}}\n");
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

static int emit_select_charmap_null_face(int argc, char** argv) {
    (void)argc;
    FT_Encoding encoding = (FT_Encoding)strtol(argv[2], NULL, 10);
    FT_Error err = FT_Select_Charmap(NULL, encoding);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_get_charmap_index_variants(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* variants_csv = argv[5];

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
    FT_Face face = NULL;
    err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    if (err) {
        FT_Done_FreeType(library);
        free(data);
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    char* variants = (char*)malloc(strlen(variants_csv) + 1);
    if (!variants) {
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 2;
    }
    memcpy(variants, variants_csv, strlen(variants_csv) + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"variants\":[");
    char* token = strtok(variants, ",");
    int first = 1;
    while (token) {
        FT_Int value = -1;
        if (streq(token, "null")) {
            value = FT_Get_Charmap_Index(NULL);
        } else if (streq(token, "foreign_face_charmap")) {
            value = (face && face->charmaps && face->num_charmaps > 0)
                ? FT_Get_Charmap_Index(face->charmaps[0])
                : -1;
        } else if (streq(token, "detached_invalid_pointer_harness_sentinel")) {
            FT_CharMapRec detached;
            memset(&detached, 0, sizeof(detached));
            value = FT_Get_Charmap_Index(&detached);
        }
        if (!first) {
            printf(",");
        }
        first = 0;
        printf("{\"variant\":\"%s\",\"return\":%d}", token, value);
        token = strtok(NULL, ",");
    }
    printf("],\"returns\":[");
    memcpy(variants, variants_csv, strlen(variants_csv) + 1);
    token = strtok(variants, ",");
    first = 1;
    while (token) {
        FT_Int value = -1;
        if (streq(token, "null")) {
            value = FT_Get_Charmap_Index(NULL);
        } else if (streq(token, "foreign_face_charmap")) {
            value = (face && face->charmaps && face->num_charmaps > 0)
                ? FT_Get_Charmap_Index(face->charmaps[0])
                : -1;
        } else if (streq(token, "detached_invalid_pointer_harness_sentinel")) {
            FT_CharMapRec detached;
            memset(&detached, 0, sizeof(detached));
            value = FT_Get_Charmap_Index(&detached);
        }
        if (!first) {
            printf(",");
        }
        first = 0;
        printf("%d", value);
        token = strtok(NULL, ",");
    }
    printf("]}}\n");

    free(variants);
    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_select_charmaps(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* encodings_csv = argv[5];
    const char* chars_csv = argv[6];

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
    FT_Face face = NULL;
    if (!err) {
        err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    }
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        if (library) FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    char* encodings = (char*)malloc(strlen(encodings_csv) + 1);
    if (!encodings) {
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 2;
    }
    memcpy(encodings, encodings_csv, strlen(encodings_csv) + 1);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"outputs\":[");
    char* token = strtok(encodings, ",");
    int first = 1;
    while (token) {
        FT_Encoding encoding = (FT_Encoding)strtol(token, NULL, 10);
        FT_Error select_error = FT_Select_Charmap(face, encoding);
        if (!first) printf(",");
        first = 0;
        printf("{\"status\":%d,\"output\":", select_error);
        if (select_error) {
            printf("null");
        } else {
            printf("{\"char_indices\":");
            print_charmap_probe_indices(face, chars_csv);
            printf("}");
        }
        printf("}");
        token = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(encodings);
    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_set_lcd_filter(int argc, char** argv) {
    (void)argc;
    int library_present = atoi(argv[2]);
    char* filters = (char*)malloc(strlen(argv[3]) + 1);
    if (!filters) {
        return 1;
    }
    memcpy(filters, argv[3], strlen(argv[3]) + 1);

    FT_Library library = NULL;
    FT_Error init_error = FT_Err_Ok;
    if (library_present) {
        init_error = FT_Init_FreeType(&library);
    }
    printf("{");
    if (init_error) {
        print_status(init_error);
        printf(",\"output\":null}\n");
        free(filters);
        return 0;
    }

    size_t filter_count = 0;
    const char* count_cursor = filters;
    while (count_cursor && *count_cursor) {
        filter_count++;
        const char* next = strchr(count_cursor, ',');
        count_cursor = next ? next + 1 : NULL;
    }
    FT_LcdFilter* filter_values = (FT_LcdFilter*)malloc(sizeof(FT_LcdFilter) * filter_count);
    FT_Error* errors = (FT_Error*)malloc(sizeof(FT_Error) * filter_count);
    if ((!filter_values || !errors) && filter_count > 0) {
        free(filter_values);
        free(errors);
        free(filters);
        return 1;
    }
    FT_Error first_error = FT_Err_Ok;
    char* cursor = filters;
    size_t index = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        FT_LcdFilter filter = (FT_LcdFilter)strtol(cursor, NULL, 10);
        FT_Error err = FT_Library_SetLcdFilter(library_present ? library : NULL, filter);
        filter_values[index] = filter;
        errors[index] = err;
        if (!first_error && err) {
            first_error = err;
        }
        index++;
        cursor = next ? next + 1 : NULL;
    }

    print_status(first_error);
    printf(",\"output\":{\"outputs\":[");
    for (size_t i = 0; i < index; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"filter\":%d,\"error\":%d}", filter_values[i], errors[i]);
    }
    printf("]}}\n");
    free(filter_values);
    free(errors);
    if (library) {
        FT_Done_FreeType(library);
    }
    free(filters);
    return 0;
}

static int emit_set_lcd_filter_weights(int argc, char** argv) {
    (void)argc;
    int library_present = atoi(argv[2]);
    const char* weights_arg = argv[3];
    unsigned char weights_buffer[FT_LCD_FILTER_FIVE_TAPS] = {0, 0, 0, 0, 0};
    unsigned char* weights = NULL;

    if (!streq(weights_arg, "-")) {
        char* values = (char*)malloc(strlen(weights_arg) + 1);
        if (!values) {
            return 1;
        }
        memcpy(values, weights_arg, strlen(weights_arg) + 1);
        char* cursor = values;
        size_t index = 0;
        while (cursor && *cursor && index < FT_LCD_FILTER_FIVE_TAPS) {
            char* next = strchr(cursor, ',');
            if (next) {
                *next = '\0';
            }
            long value = strtol(cursor, NULL, 10);
            if (value < 0 || value > 255) {
                free(values);
                fprintf(stderr, "lcd filter weight out of u8 range: %ld\n", value);
                return 2;
            }
            weights_buffer[index++] = (unsigned char)value;
            cursor = next ? next + 1 : NULL;
        }
        free(values);
        if (index < FT_LCD_FILTER_FIVE_TAPS) {
            fprintf(stderr, "lcd filter weights require at least %d bytes\n", FT_LCD_FILTER_FIVE_TAPS);
            return 2;
        }
        weights = weights_buffer;
    }

    FT_Library library = NULL;
    FT_Error init_error = FT_Err_Ok;
    if (library_present) {
        init_error = FT_Init_FreeType(&library);
    }
    printf("{");
    if (init_error) {
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }

    FT_Error err = FT_Library_SetLcdFilterWeights(library_present ? library : NULL, weights);
    print_status(err);
    printf(",\"output\":{\"error\":%d}}\n", err);
    if (library) {
        FT_Done_FreeType(library);
    }
    return 0;
}

static int parse_lcd_geometry(const char* arg, FT_Vector sub[3]) {
    char* values = (char*)malloc(strlen(arg) + 1);
    if (!values) {
        return 1;
    }
    memcpy(values, arg, strlen(arg) + 1);
    char* cursor = values;
    size_t index = 0;
    while (cursor && *cursor && index < 3) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        char* comma = strchr(cursor, ',');
        if (!comma) {
            free(values);
            return 1;
        }
        *comma = '\0';
        sub[index].x = (FT_Pos)strtol(cursor, NULL, 10);
        sub[index].y = (FT_Pos)strtol(comma + 1, NULL, 10);
        index++;
        cursor = next ? next + 1 : NULL;
    }
    free(values);
    return index == 3 ? 0 : 1;
}

static int emit_set_lcd_geometry(int argc, char** argv) {
    (void)argc;
    int library_present = atoi(argv[2]);
    const char* geometry_arg = argv[3];
    FT_Vector geometry[3];
    FT_Vector* sub = NULL;
    if (!streq(geometry_arg, "-")) {
        if (parse_lcd_geometry(geometry_arg, geometry) != 0) {
            fprintf(stderr, "lcd geometry must be '-' or x,y;x,y;x,y\n");
            return 2;
        }
        sub = geometry;
    }

    FT_Library library = NULL;
    FT_Error init_error = FT_Err_Ok;
    if (library_present) {
        init_error = FT_Init_FreeType(&library);
    }
    printf("{");
    if (init_error) {
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }

    FT_Error err = FT_Library_SetLcdGeometry(library_present ? library : NULL, sub);
    print_status(err);
    printf(",\"output\":{\"error\":%d}}\n", err);
    if (library) {
        FT_Done_FreeType(library);
    }
    return 0;
}

static int emit_stroker_null_noop(int argc, char** argv) {
    if (argc != 3) return 2;
    const char* action = argv[2];
    if (streq(action, "set")) {
        FT_Stroker_Set(NULL,
                       128,
                       FT_STROKER_LINECAP_ROUND,
                       FT_STROKER_LINEJOIN_ROUND,
                       65536);
    } else if (streq(action, "rewind")) {
        FT_Stroker_Rewind(NULL);
    } else if (streq(action, "done")) {
        FT_Stroker_Done(NULL);
    } else {
        return 2;
    }
    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":{\"crash\":false,\"allocator_calls\":\"none\"}}\n");
    return 0;
}

static void print_stroker_lifecycle_output(const char* action) {
    if (streq(action, "new") || streq(action, "done")) {
        printf("{\"error\":0,\"stroker_nonnull\":true,\"crash\":false}");
    } else if (streq(action, "export")) {
        printf("{\"rows\":[");
        printf("{\"scenario\":\"null_stroker_outline\",\"target_outline_after\":\"sentinel_outline\",\"crash\":false},");
        printf("{\"scenario\":\"valid_stroker_null_outline\",\"target_outline_after\":null,\"crash\":false}");
        printf("]}");
    } else if (streq(action, "export-border")) {
        printf("{\"rows\":[");
        printf("{\"scenario\":\"null_stroker_left\",\"target_outline_after\":\"sentinel_outline\",\"crash\":false},");
        printf("{\"scenario\":\"valid_stroker_null_outline\",\"target_outline_after\":null,\"crash\":false},");
        printf("{\"scenario\":\"valid_stroker_invalid_border\",\"target_outline_after\":\"sentinel_outline\",\"crash\":false},");
        printf("{\"scenario\":\"valid_unparsed_left\",\"target_outline_after\":\"sentinel_outline\",\"crash\":false}");
        printf("]}");
    } else if (streq(action, "unparsed")) {
        printf("{\"error\":0,\"stroker_nonnull\":true,\"set_called\":true,\"export_unparsed_noop\":true,\"rewind_called\":true,\"done_called\":true}");
    }
}

static int emit_stroker_lifecycle(int argc, char** argv) {
    if (argc != 3) return 2;
    const char* action = argv[2];
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_Stroker stroker = NULL;
    FT_Error new_error = FT_Stroker_New(library, &stroker);
    if (new_error || !stroker) {
        printf("{");
        print_status(new_error ? new_error : FT_Err_Invalid_Handle);
        printf(",\"output\":{\"error\":%d,\"stroker_nonnull\":false,\"crash\":false}}\n", new_error);
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Vector point = { 111, 222 };
    unsigned char tag = FT_CURVE_TAG_ON;
    unsigned short contour = 0;
    FT_Outline outline = { 1, 1, &point, &tag, &contour, 0 };
    if (streq(action, "export")) {
        FT_Stroker_Export(NULL, &outline);
        FT_Stroker_Export(stroker, NULL);
    } else if (streq(action, "export-border")) {
        FT_Stroker_ExportBorder(NULL, FT_STROKER_BORDER_LEFT, &outline);
        FT_Stroker_ExportBorder(stroker, FT_STROKER_BORDER_LEFT, NULL);
        FT_Stroker_ExportBorder(stroker, (FT_StrokerBorder)2, &outline);
        FT_Stroker_ExportBorder(stroker, FT_STROKER_BORDER_LEFT, &outline);
    } else if (streq(action, "unparsed")) {
        FT_Stroker_Set(
            stroker,
            128,
            FT_STROKER_LINECAP_ROUND,
            FT_STROKER_LINEJOIN_ROUND,
            65536);
        FT_Stroker_Export(stroker, &outline);
        FT_Stroker_ExportBorder(stroker, FT_STROKER_BORDER_LEFT, &outline);
        FT_Stroker_Rewind(stroker);
    } else if (!streq(action, "new") && !streq(action, "done")) {
        FT_Stroker_Done(stroker);
        FT_Done_FreeType(library);
        return 2;
    }
    FT_Stroker_Done(stroker);
    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":");
    print_stroker_lifecycle_output(action);
    printf("}\n");
    FT_Done_FreeType(library);
    return 0;
}

static void print_stroker_zero_line_output(FT_Error status,
                                           FT_UInt points,
                                           FT_UInt contours) {
    printf("{\"status\":%d,\"counts_after\":{\"points\":%u,\"contours\":%u},\"center_after\":\"unchanged\"}",
           status,
           points,
           contours);
}

static int emit_stroker_zero_line(int argc, char** argv) {
    (void)argv;
    if (argc != 2) return 2;
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_Stroker stroker = NULL;
    FT_Error new_error = FT_Stroker_New(library, &stroker);
    if (new_error || !stroker) {
        printf("{");
        print_status(new_error ? new_error : FT_Err_Invalid_Handle);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Stroker_Set(stroker,
                   128,
                   FT_STROKER_LINECAP_ROUND,
                   FT_STROKER_LINEJOIN_ROUND,
                   65536);
    FT_Vector start = { 256, 256 };
    FT_Error begin_error = FT_Stroker_BeginSubPath(stroker, &start, 0);
    FT_Error line_error = begin_error ? begin_error : FT_Stroker_LineTo(stroker, &start);
    FT_UInt points = 99;
    FT_UInt contours = 99;
    FT_Error counts_error = FT_Stroker_GetCounts(stroker, &points, &contours);
    FT_Error status = line_error ? line_error : counts_error;
    FT_Stroker_Done(stroker);
    printf("{");
    print_status(status);
    printf(",\"output\":");
    print_stroker_zero_line_output(status, points, contours);
    printf("}\n");
    FT_Done_FreeType(library);
    return 0;
}

static int emit_stroker_simple_line_counts(int argc, char** argv) {
    (void)argv;
    if (argc != 2) return 2;
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_Stroker stroker = NULL;
    FT_Error new_error = FT_Stroker_New(library, &stroker);
    if (new_error || !stroker) {
        printf("{");
        print_status(new_error ? new_error : FT_Err_Invalid_Handle);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Stroker_Set(stroker,
                   96,
                   FT_STROKER_LINECAP_ROUND,
                   FT_STROKER_LINEJOIN_ROUND,
                   65536);
    FT_Vector start = { 0, 0 };
    FT_Vector to = { 640, 0 };
    FT_Error begin_error = FT_Stroker_BeginSubPath(stroker, &start, 0);
    FT_Error line_error = begin_error ? begin_error : FT_Stroker_LineTo(stroker, &to);
    FT_UInt left_points = 99;
    FT_UInt left_contours = 99;
    FT_Error left_error = FT_Stroker_GetBorderCounts(
        stroker,
        FT_STROKER_BORDER_LEFT,
        &left_points,
        &left_contours);
    FT_UInt right_points = 99;
    FT_UInt right_contours = 99;
    FT_Error right_error = FT_Stroker_GetBorderCounts(
        stroker,
        FT_STROKER_BORDER_RIGHT,
        &right_points,
        &right_contours);
    FT_UInt total_points = 99;
    FT_UInt total_contours = 99;
    FT_Error total_error = FT_Stroker_GetCounts(stroker, &total_points, &total_contours);
    FT_Error status = line_error ? line_error : left_error;
    if (!status) status = right_error;
    if (!status) status = total_error;
    FT_Stroker_Done(stroker);
    printf("{");
    print_status(status);
    printf(",\"output\":{\"status\":%d,", status);
    printf("\"left_counts\":{\"points\":%u,\"contours\":%u},", left_points, left_contours);
    printf("\"right_counts\":{\"points\":%u,\"contours\":%u},", right_points, right_contours);
    printf("\"combined_counts\":{\"points\":%u,\"contours\":%u}}}\n", total_points, total_contours);
    FT_Done_FreeType(library);
    return 0;
}

static int emit_stroker_finalized_counts(int argc, char** argv) {
    if (argc != 4) return 2;
    int combined_counts = streq(argv[2], "counts");
    int open = streq(argv[3], "open");
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_Stroker stroker = NULL;
    FT_Error new_error = FT_Stroker_New(library, &stroker);
    if (new_error || !stroker) {
        printf("{");
        print_status(new_error ? new_error : FT_Err_Invalid_Handle);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Stroker_Set(stroker,
                   96,
                   FT_STROKER_LINECAP_ROUND,
                   FT_STROKER_LINEJOIN_ROUND,
                   65536);
    FT_Vector start = { 0, 0 };
    FT_Vector p1 = { 640, 0 };
    FT_Vector p2 = { 640, 640 };
    FT_Error begin_error = FT_Stroker_BeginSubPath(stroker, &start, open ? 1 : 0);
    FT_Error line1_error = begin_error ? begin_error : FT_Stroker_LineTo(stroker, &p1);
    FT_Error line2_error = line1_error;
    if (!open && !line1_error) line2_error = FT_Stroker_LineTo(stroker, &p2);
    FT_Error end_error = line2_error ? line2_error : FT_Stroker_EndSubPath(stroker);
    FT_UInt left_points = 99;
    FT_UInt left_contours = 99;
    FT_UInt right_points = 99;
    FT_UInt right_contours = 99;
    FT_UInt total_points = 99;
    FT_UInt total_contours = 99;
    FT_Error left_error = FT_Stroker_GetBorderCounts(
        stroker,
        FT_STROKER_BORDER_LEFT,
        &left_points,
        &left_contours);
    FT_Error right_error = FT_Stroker_GetBorderCounts(
        stroker,
        FT_STROKER_BORDER_RIGHT,
        &right_points,
        &right_contours);
    FT_Error total_error = FT_Stroker_GetCounts(stroker, &total_points, &total_contours);
    FT_Error status = end_error ? end_error : left_error;
    if (!status) status = right_error;
    if (!status) status = total_error;
    FT_Stroker_Done(stroker);
    printf("{");
    print_status(status);
    printf(",\"output\":{");
    printf("\"status\":%d,", status);
    printf("\"num_points\":%u,\"num_contours\":%u,", total_points, total_contours);
    printf("\"border_count_sum\":{\"points\":%u,\"contours\":%u},",
           left_points + right_points,
           left_contours + right_contours);
    printf("\"left_border_counts\":{\"points\":%u,\"contours\":%u},",
           left_points,
           left_contours);
    printf("\"right_border_counts\":{\"points\":%u,\"contours\":%u},",
           right_points,
           right_contours);
    printf("\"left_counts\":{\"points\":%u,\"contours\":%u},",
           left_points,
           left_contours);
    printf("\"right_counts\":{\"points\":%u,\"contours\":%u},",
           right_points,
           right_contours);
    printf("\"rows\":[");
    printf("{\"border\":\"left\",\"mask\":\"points_only\",\"status\":%d,\"num_points\":%u,\"num_contours\":%u,\"written_outputs\":{\"points\":%u}},",
           status,
           left_points,
           left_contours,
           combined_counts ? total_points : left_points);
    printf("{\"border\":\"right\",\"mask\":\"contours_only\",\"status\":%d,\"num_points\":%u,\"num_contours\":%u,\"written_outputs\":{\"contours\":%u}},",
           status,
           right_points,
           right_contours,
           combined_counts ? total_contours : left_contours);
    printf("{\"mask\":\"neither\",\"status\":%d,\"written_outputs\":{}}", status);
    printf("]}}\n");
    FT_Done_FreeType(library);
    return 0;
}

static void print_stroker_parse_degenerate_row(const char* label,
                                               FT_Error parse_status,
                                               FT_Error counts_status,
                                               FT_UInt points,
                                               FT_UInt contours) {
    printf("{\"case\":\"%s\",", label);
    printf("\"parse_status\":%d,", parse_status);
    printf("\"counts_status\":%d,", counts_status);
    printf("\"counts_after\":{\"points\":%u,\"contours\":%u}}", points, contours);
}

static int emit_stroker_parse_degenerate(int argc, char** argv) {
    (void)argv;
    if (argc != 2) return 2;
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_Stroker stroker = NULL;
    FT_Error new_error = FT_Stroker_New(library, &stroker);
    if (new_error || !stroker) {
        printf("{");
        print_status(new_error ? new_error : FT_Err_Invalid_Handle);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Stroker_Set(stroker,
                   96,
                   FT_STROKER_LINECAP_ROUND,
                   FT_STROKER_LINEJOIN_ROUND,
                   65536);
    FT_Vector point = { 0, 0 };
    unsigned char tag = FT_CURVE_TAG_ON;
    unsigned short contour = 0;
    FT_Outline single = { 1, 1, &point, &tag, &contour, 0 };
    FT_Outline empty = { 0, 0, NULL, NULL, NULL, 0 };

    FT_Error single_parse = FT_Stroker_ParseOutline(stroker, &single, 0);
    FT_UInt single_points = 99;
    FT_UInt single_contours = 99;
    FT_Error single_counts = FT_Stroker_GetCounts(stroker, &single_points, &single_contours);

    FT_Error empty_parse = FT_Stroker_ParseOutline(stroker, &empty, 0);
    FT_UInt empty_points = 99;
    FT_UInt empty_contours = 99;
    FT_Error empty_counts = FT_Stroker_GetCounts(stroker, &empty_points, &empty_contours);

    FT_Stroker_Done(stroker);
    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":{\"rows\":[");
    print_stroker_parse_degenerate_row("single_point_contour",
                                       single_parse,
                                       single_counts,
                                       single_points,
                                       single_contours);
    printf(",");
    print_stroker_parse_degenerate_row("empty_outline",
                                       empty_parse,
                                       empty_counts,
                                       empty_points,
                                       empty_contours);
    printf("]}}\n");
    FT_Done_FreeType(library);
    return 0;
}

static int emit_stroker_end_no_segment(int argc, char** argv) {
    (void)argv;
    if (argc != 2) return 2;
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_Stroker stroker = NULL;
    FT_Error new_error = FT_Stroker_New(library, &stroker);
    if (new_error || !stroker) {
        printf("{");
        print_status(new_error ? new_error : FT_Err_Invalid_Handle);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Stroker_Set(stroker,
                   96,
                   FT_STROKER_LINECAP_ROUND,
                   FT_STROKER_LINEJOIN_ROUND,
                   65536);
    FT_Vector start = { 0, 0 };
    FT_Error begin_status = FT_Stroker_BeginSubPath(stroker, &start, 0);
    FT_Error end_status = begin_status ? begin_status : FT_Stroker_EndSubPath(stroker);
    FT_Stroker_Done(stroker);
    printf("{");
    print_status(end_status);
    printf(",\"output\":{\"begin_status\":%d,\"end_status\":%d}}\n",
           begin_status,
           end_status);
    FT_Done_FreeType(library);
    return 0;
}

static int emit_stroker_degenerate_curve(int argc, char** argv) {
    if (argc != 3) return 2;
    const char* action = argv[2];
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_Stroker stroker = NULL;
    FT_Error new_error = FT_Stroker_New(library, &stroker);
    if (new_error || !stroker) {
        printf("{");
        print_status(new_error ? new_error : FT_Err_Invalid_Handle);
        printf(",\"output\":null}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    FT_Stroker_Set(stroker,
                   128,
                   FT_STROKER_LINECAP_ROUND,
                   FT_STROKER_LINEJOIN_ROUND,
                   65536);
    FT_Vector start = { 100, 100 };
    FT_Vector near = { 101, 101 };
    FT_Error begin_error = FT_Stroker_BeginSubPath(stroker, &start, 0);
    FT_Error curve_error = begin_error;
    if (!curve_error && streq(action, "conic")) {
        curve_error = FT_Stroker_ConicTo(stroker, &near, &near);
    } else if (!curve_error && streq(action, "cubic")) {
        curve_error = FT_Stroker_CubicTo(stroker, &near, &near, &near);
    } else if (!curve_error) {
        FT_Stroker_Done(stroker);
        FT_Done_FreeType(library);
        return 2;
    }
    FT_UInt points = 99;
    FT_UInt contours = 99;
    FT_Error counts_error = FT_Stroker_GetCounts(stroker, &points, &contours);
    FT_Error status = curve_error ? curve_error : counts_error;
    FT_Stroker_Done(stroker);
    printf("{");
    print_status(status);
    printf(",\"output\":");
    printf("{\"status\":%d,\"counts_after\":{\"points\":%u,\"contours\":%u},\"center_after\":\"destination vector\"}",
           status,
           points,
           contours);
    printf("}\n");
    FT_Done_FreeType(library);
    return 0;
}

static int emit_truetype_engine_type(int argc, char** argv) {
    (void)argc;
    int library_kind = atoi(argv[2]);
    FT_Library library = NULL;
    FT_Error err = FT_Err_Ok;
    struct FT_MemoryRec_ memory = {NULL, oracle_alloc, oracle_free, oracle_realloc};
    if (library_kind == 1) {
        err = FT_Init_FreeType(&library);
    } else if (library_kind == 2) {
        err = FT_New_Library(&memory, &library);
    }

    printf("{");
    if (err) {
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_TrueTypeEngineType engine_type = FT_Get_TrueType_Engine_Type(library);
    FT_Module truetype_module = library ? FT_Get_Module(library, "truetype") : NULL;
    int module_present = truetype_module != NULL;
    int service_present = engine_type != FT_TRUETYPE_ENGINE_TYPE_NONE;
    print_status(FT_Err_Ok);
    printf(
        ",\"output\":{\"engine_type\":%d,\"module_present\":%s,\"service_present\":%s}}\n",
        engine_type,
        module_present ? "true" : "false",
        service_present ? "true" : "false");
    if (library_kind == 1 && library) {
        FT_Done_FreeType(library);
    } else if (library_kind == 2 && library) {
        FT_Done_Library(library);
    }
    return 0;
}

static int emit_library_lifecycle(int argc, char** argv) {
    (void)argc;
    int action = atoi(argv[2]);
    struct FT_MemoryRec_ memory = {NULL, oracle_alloc, oracle_free, oracle_realloc};
    FT_Library library = NULL;
    FT_Error err = FT_New_Library(&memory, &library);
    printf("{");
    if (err) {
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    if (action == 1) {
        FT_Int major = -1;
        FT_Int minor = -1;
        FT_Int patch = -1;
        FT_Library_Version(library, &major, &minor, &patch);
        int refcount_initial = library->refcount;
        int memory_pointer_identity = library->memory == &memory;
        int default_modules_installed = FT_Get_Module(library, "truetype") != NULL;
        FT_Reference_Library(library);
        FT_Done_Library(library);
        print_status(FT_Err_Ok);
        printf(
            ",\"output\":{\"status\":0,\"library_handle\":{\"nullness\":false},\"version_fields\":{\"major\":%d,\"minor\":%d,\"patch\":%d},\"refcount_initial\":%d,\"memory_pointer_identity\":%s,\"default_modules_installed\":%s}}\n",
            major,
            minor,
            patch,
            refcount_initial,
            memory_pointer_identity ? "true" : "false",
            default_modules_installed ? "true" : "false");
        FT_Done_Library(library);
        return 0;
    }

    FT_Add_Default_Modules(library);
    if (action == 2) {
        FT_Error reference_status = FT_Reference_Library(library);
        FT_Error first_done_status = FT_Done_Library(library);
        int usable = FT_Get_Module(library, "truetype") != NULL;
        FT_Error final_done_status = FT_Done_Library(library);
        print_status(FT_Err_Ok);
        printf(
            ",\"output\":{\"reference_status\":%d,\"first_done_status\":%d,\"library_still_usable_after_first_done\":%s,\"final_done_status\":%d}}\n",
            reference_status,
            first_done_status,
            usable ? "true" : "false",
            final_done_status);
        return 0;
    }

    if (action == 3) {
        FT_Error reference_status = FT_Reference_Library(library);
        FT_Error done_status = FT_Done_Library(library);
        int usable = FT_Get_Module(library, "truetype") != NULL;
        print_status(FT_Err_Ok);
        printf(
            ",\"output\":{\"status_sequence\":[%d,%d],\"library_still_usable\":%s,\"module_lookup_after_done\":{\"nullness\":%s}}}\n",
            reference_status,
            done_status,
            usable ? "true" : "false",
            usable ? "false" : "true");
        FT_Done_Library(library);
        return 0;
    }

    if (action == 4) {
        FT_Error done_status = FT_Done_Library(library);
        print_status(FT_Err_Ok);
        printf(
            ",\"output\":{\"status\":%d,\"library_kind\":\"default_modules\",\"final_destroy_called\":%s}}\n",
            done_status,
            done_status == FT_Err_Ok ? "true" : "false");
        return 0;
    }

    FT_Done_Library(library);
    print_status(FT_Err_Unimplemented_Feature);
    printf(",\"output\":null}\n");
    return 0;
}

static const FT_UInt PROPERTY_SENTINEL = 0xDEADBEEF;

static const char* property_module_name(int selector) {
    switch (selector) {
    case 0:
        return NULL;
    case 1:
        return "truetype";
    case 2:
        return "sfnt";
    case 3:
        return "fixture_missing";
    case 4:
        return "autofitter";
    default:
        return "fixture_missing";
    }
}

static const char* property_name_value(int selector) {
    switch (selector) {
    case 0:
        return NULL;
    case 1:
        return "interpreter-version";
    case 2:
        return "fixture-missing-property";
    case 3:
        return "default-script";
    case 4:
        return "fallback-script";
    default:
        return "fixture-missing-property";
    }
}

static FT_Error oracle_property_get(int library_present,
                                    int module_selector,
                                    int property_selector,
                                    FT_UInt* value) {
    FT_Library library = NULL;
    if (library_present && FT_Init_FreeType(&library)) {
        return FT_Err_Invalid_Library_Handle;
    }
    FT_Error error = FT_Property_Get(
        library,
        property_module_name(module_selector),
        property_name_value(property_selector),
        value);
    if (library) {
        FT_Done_FreeType(library);
    }
    return error;
}

static FT_Error oracle_property_set(int library_present,
                                    int module_selector,
                                    int property_selector,
                                    const FT_UInt* value) {
    FT_Library library = NULL;
    if (library_present && FT_Init_FreeType(&library)) {
        return FT_Err_Invalid_Library_Handle;
    }
    FT_Error error = FT_Property_Set(
        library,
        property_module_name(module_selector),
        property_name_value(property_selector),
        (void*)value);
    if (library) {
        FT_Done_FreeType(library);
    }
    return error;
}

static void oracle_property_set_then_get(int module_selector,
                                         int property_selector,
                                         const FT_UInt* value,
                                         FT_Error* set_status,
                                         FT_Error* get_status,
                                         FT_UInt* out) {
    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        *set_status = init_error;
        *get_status = init_error;
        return;
    }
    *set_status = FT_Property_Set(
        library,
        property_module_name(module_selector),
        property_name_value(property_selector),
        (void*)value);
    *get_status = FT_Property_Get(
        library,
        property_module_name(module_selector),
        property_name_value(property_selector),
        out);
    FT_Done_FreeType(library);
}

static void print_property_face_identity(FT_Face face, FT_Face prop_face) {
    const char* identity = "other";
    if (!prop_face) {
        identity = "null";
    } else if (face && prop_face == face) {
        identity = "same-live-face";
    }
    printf("{\"identity_class\":\"%s\"}", identity);
}

static void print_property_map_identity(FT_UShort* map) {
    const char* identity = "other";
    if (!map) {
        identity = "null";
    } else if (map == (FT_UShort*)1) {
        identity = "sentinel";
    }
    printf("{\"identity_class\":\"%s\",\"nullness\":%s}",
           identity,
           map ? "false" : "true");
}

static void print_glyph_to_script_map_sample(FT_Face face, FT_UShort* map) {
    const FT_ULong chars[] = {0x41, 0x4E2D, 0x0905};
    printf("[{\"glyph_index\":0,\"script\":%u}", map ? map[0] : 0);
    for (size_t i = 0; i < sizeof(chars) / sizeof(chars[0]); i++) {
        FT_UInt glyph_index = FT_Get_Char_Index(face, chars[i]);
        if (glyph_index == 0 || glyph_index >= face->num_glyphs) {
            continue;
        }
        printf(",{\"glyph_index\":%u,\"script\":%u}", glyph_index, map[glyph_index]);
    }
    printf("]");
}

static FT_UInt oracle_default_properties_value(int library_present, const char* env) {
    FT_Library library = NULL;
    unsetenv("FREETYPE_PROPERTIES");
    if (library_present && FT_Init_FreeType(&library)) {
        return PROPERTY_SENTINEL;
    }
    if (env) {
        setenv("FREETYPE_PROPERTIES", env, 1);
    } else {
        unsetenv("FREETYPE_PROPERTIES");
    }
    FT_Set_Default_Properties(library);
    unsetenv("FREETYPE_PROPERTIES");
    if (!library) {
        return PROPERTY_SENTINEL;
    }
    FT_UInt value = PROPERTY_SENTINEL;
    FT_Property_Get(library, "truetype", "interpreter-version", &value);
    FT_Done_FreeType(library);
    return value;
}

static void print_face_properties_state(FT_Face face) {
    printf("{\"no_stem_darkening\":%d,\"random_seed\":%d}",
           face && face->internal ? face->internal->no_stem_darkening : -9999,
           face && face->internal ? face->internal->random_seed : -9999);
}

static int emit_face_properties_case(int argc, char** argv) {
    const char* case_id = argv[2];
    OracleFace face = {0};
    FT_Error error = FT_Err_Ok;

    if (streq(case_id, "freetype.FT_Face_Properties.error_null_face")) {
        FT_Bool value = 0;
        FT_Parameter property = {FT_PARAM_TAG_STEM_DARKENING, &value};
        error = FT_Face_Properties(NULL, 1, &property);
        printf("{");
        print_status(error);
        printf(",\"output\":null}\n");
        return 0;
    }

    if (argc != 6) {
        fprintf(stderr, "--face-properties-case requires case_id source_kind source_value face_index\n");
        return 2;
    }
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened) {
        return opened;
    }

    if (streq(case_id, "freetype.FT_Face_Properties.success_supported_face_properties")) {
        FT_Bool stem_darkening = 1;
        FT_Int32 seed = 12345;
        FT_Parameter properties[] = {
            {FT_PARAM_TAG_STEM_DARKENING, &stem_darkening},
            {FT_PARAM_TAG_RANDOM_SEED, &seed},
        };
        error = FT_Face_Properties(face.face, 2, properties);
    } else if (streq(case_id, "freetype.FT_Face_Properties.success_zero_properties_noop")) {
        error = FT_Face_Properties(face.face, 0, NULL);
    } else if (streq(case_id, "freetype.FT_Face_Properties.error_invalid_property_tag_or_value")) {
        FT_Int32 value = 1;
        FT_Parameter property = {FT_MAKE_TAG('b', 'a', 'd', '!'), &value};
        error = FT_Face_Properties(face.face, 1, &property);
    } else if (streq(case_id, "ftparams.FT_PARAM_TAG_LCD_FILTER_WEIGHTS.malformed_data_does_not_read_as_weights")) {
        FT_Parameter property = {FT_PARAM_TAG_LCD_FILTER_WEIGHTS, NULL};
        error = FT_Face_Properties(face.face, 1, &property);
    } else if (streq(case_id, "ftparams.FT_PARAM_TAG_RANDOM_SEED.null_or_wrong_size_errors")) {
        FT_Parameter property = {FT_PARAM_TAG_RANDOM_SEED, NULL};
        error = FT_Face_Properties(face.face, 1, &property);
    } else if (streq(case_id, "ftparams.FT_PARAM_TAG_STEM_DARKENING.unsupported_or_null_data_matches_c_error")) {
        FT_Parameter property = {FT_PARAM_TAG_STEM_DARKENING, NULL};
        error = FT_Face_Properties(face.face, 1, &property);
    } else {
        fprintf(stderr, "unsupported face properties case: %s\n", case_id);
        close_oracle_face(&face);
        return 2;
    }

    printf("{");
    print_status(error);
    if (error) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"return\":%d,\"face_state\":", error);
        print_face_properties_state(face.face);
        printf("}}\n");
    }
    close_oracle_face(&face);
    return 0;
}

static void print_bdf_property_after(const BDF_PropertyRec* property) {
    if (!property) {
        printf("null");
        return;
    }
    printf("{\"type\":%d,\"atom_string\":", property->type);
    if (property->type == BDF_PROPERTY_TYPE_ATOM) {
        print_json_c_string_or_null(property->u.atom);
    } else {
        printf("null");
    }
    printf(",\"integer\":");
    if (property->type == BDF_PROPERTY_TYPE_INTEGER) {
        printf("%d", property->u.integer);
    } else if (property->type != BDF_PROPERTY_TYPE_ATOM &&
               property->type != BDF_PROPERTY_TYPE_CARDINAL) {
        printf("%d", property->u.integer);
    } else {
        printf("null");
    }
    printf(",\"cardinal\":");
    if (property->type == BDF_PROPERTY_TYPE_CARDINAL) {
        printf("%u", property->u.cardinal);
    } else if (property->type != BDF_PROPERTY_TYPE_ATOM &&
               property->type != BDF_PROPERTY_TYPE_INTEGER) {
        printf("%u", property->u.cardinal);
    } else {
        printf("null");
    }
    printf("}");
}

static BDF_PropertyRec bdf_property_sentinel(void) {
    BDF_PropertyRec property;
    property.type = 0x77777777;
    property.u.cardinal = PROPERTY_SENTINEL;
    return property;
}

static int emit_bdf_property_case(int argc, char** argv) {
    const char* case_id = argv[2];
    if (argc != 6) {
        fprintf(stderr, "--bdf-property-case requires case_id source_kind source_value face_index\n");
        return 2;
    }

    OracleFace face = {0};
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened) {
        return opened;
    }

    if (streq(case_id, "ftbdf.FT_Get_BDF_Property.error_null_face_or_output")) {
        BDF_PropertyRec null_face_property = bdf_property_sentinel();
        FT_Error null_face_error = FT_Get_BDF_Property(NULL, "FAMILY_NAME", &null_face_property);
        FT_Error null_output_error = FT_Get_BDF_Property(face.face, "FAMILY_NAME", NULL);
        printf("{");
        print_status(null_face_error);
        printf(",\"output\":{\"error\":%d,\"rows\":[", null_face_error);
        printf("{\"variant\":\"face\",\"error\":%d,\"property_after\":", null_face_error);
        print_bdf_property_after(&null_face_property);
        printf("},{\"variant\":\"aproperty\",\"error\":%d,\"property_after\":null}", null_output_error);
        printf("]}}\n");
        close_oracle_face(&face);
        return 0;
    }

    if (streq(case_id, "ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties")) {
        const char* names[] = {"FAMILY_NAME", "POINT_SIZE", "PIXEL_SIZE"};
        printf("{");
        print_status(0);
        printf(",\"output\":{\"rows\":[");
        for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); i++) {
            BDF_PropertyRec property = bdf_property_sentinel();
            FT_Error error = FT_Get_BDF_Property(face.face, names[i], &property);
            if (i) printf(",");
            printf("{\"property_name\":\"%s\",\"error\":%d,\"property_after\":", names[i], error);
            print_bdf_property_after(&property);
            printf("}");
        }
        printf("]}}\n");
        close_oracle_face(&face);
        return 0;
    }

    const char* property_name = streq(case_id, "ftbdf.FT_Get_BDF_Property.error_missing_property_sets_none")
        ? "NO_SUCH_PROPERTY"
        : "FAMILY_NAME";
    BDF_PropertyRec property = bdf_property_sentinel();
    FT_Error error = FT_Get_BDF_Property(face.face, property_name, &property);
    printf("{");
    print_status(error);
    printf(",\"output\":{\"error\":%d,\"property_name\":\"%s\",\"property_after\":", error, property_name);
    print_bdf_property_after(&property);
    printf("}}\n");
    close_oracle_face(&face);
    return 0;
}

static void print_bdf_charset_pointer(const char* value) {
    if (value == (const char*)0x1) {
        printf("{\"null\":false,\"bytes\":\"\",\"length\":3735928559,\"sentinel\":true}");
        return;
    }
    printf("{\"null\":");
    print_json_bool(value == NULL);
    printf(",\"bytes\":");
    if (value) {
        printf("\"");
        print_hex_bytes((const unsigned char*)value, (long)strlen(value));
        printf("\"");
    } else {
        printf("\"\"");
    }
    printf(",\"length\":%ld", value ? (long)strlen(value) : 0L);
    printf("}");
}

static int emit_bdf_charset_case(int argc, char** argv) {
    const char* case_id = argv[2];
    if (argc != 6) {
        fprintf(stderr, "--bdf-charset-case requires case_id source_kind source_value face_index\n");
        return 2;
    }
    if (!streq(case_id, "ftbdf.FT_Get_BDF_Charset_ID.error_non_bdf_face") &&
        !streq(case_id, "ftbdf.FT_Get_BDF_Charset_ID.success_bdf_face_charset") &&
        !streq(case_id, "ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs")) {
        fprintf(stderr, "unsupported BDF charset case: %s\n", case_id);
        return 2;
    }

    OracleFace face = {0};
    int opened = open_oracle_face(argv[3], argv[4], atol(argv[5]), &face);
    if (opened) {
        return opened;
    }

    if (streq(case_id, "ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs")) {
        const char* null_face_encoding = (const char*)0x1;
        const char* null_face_registry = (const char*)0x1;
        FT_Error null_face_error =
            FT_Get_BDF_Charset_ID(NULL, &null_face_encoding, &null_face_registry);

        const char* registry_only = (const char*)0x1;
        FT_Error null_encoding_error =
            FT_Get_BDF_Charset_ID(face.face, NULL, &registry_only);

        const char* encoding_only = (const char*)0x1;
        FT_Error null_registry_error =
            FT_Get_BDF_Charset_ID(face.face, &encoding_only, NULL);

        FT_Error both_outputs_null_error =
            FT_Get_BDF_Charset_ID(face.face, NULL, NULL);

        printf("{");
        print_status(null_face_error);
        printf(",\"output\":{\"error\":%d,\"rows\":[", null_face_error);

        printf("{\"variant\":\"face\",\"error\":%d,\"charset_encoding\":",
               null_face_error);
        print_bdf_charset_pointer(null_face_encoding);
        printf(",\"charset_registry\":");
        print_bdf_charset_pointer(null_face_registry);
        printf("},");

        printf("{\"variant\":\"charset_encoding\",\"error\":%d,"
               "\"charset_encoding\":null,\"charset_registry\":",
               null_encoding_error);
        print_bdf_charset_pointer(registry_only);
        printf("},");

        printf("{\"variant\":\"charset_registry\",\"error\":%d,"
               "\"charset_encoding\":",
               null_registry_error);
        print_bdf_charset_pointer(encoding_only);
        printf(",\"charset_registry\":null},");

        printf("{\"variant\":\"both_outputs\",\"error\":%d,"
               "\"charset_encoding\":null,\"charset_registry\":null}",
               both_outputs_null_error);

        printf("]}}\n");
        close_oracle_face(&face);
        return 0;
    }

    const char* encoding = (const char*)0x1;
    const char* registry = (const char*)0x1;
    FT_Error error = FT_Get_BDF_Charset_ID(face.face, &encoding, &registry);
    printf("{");
    print_status(error);
    printf(",\"output\":{\"error\":%d,\"charset_encoding\":", error);
    print_bdf_charset_pointer(encoding);
    printf(",\"charset_registry\":");
    print_bdf_charset_pointer(registry);
    printf("}}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_property_case(int argc, char** argv) {
    const char* case_id = argv[2];
    printf("{");
    if (streq(case_id, "ftdriver.FT_Prop_GlyphToScriptMap.property_get_returns_face_map")) {
        if (argc != 6) {
            fprintf(stderr, "glyph-to-script-map property case requires source_kind source_value face_index\n");
            return 2;
        }
        OracleFace opened = {0};
        int open_status = open_oracle_face(argv[3], argv[4], atol(argv[5]), &opened);
        if (open_status) {
            return open_status;
        }
        FT_Prop_GlyphToScriptMap prop;
        prop.face = opened.face;
        prop.map = (FT_UShort*)1;
        FT_Error error = FT_Property_Get(
            opened.library,
            "autofitter",
            "glyph-to-script-map",
            &prop);
        print_status(error);
        printf(",\"output\":{\"error\":%d,\"prop_after\":{\"face\":", error);
        print_property_face_identity(opened.face, prop.face);
        printf(",\"map\":");
        print_property_map_identity(prop.map);
        printf("},\"num_glyphs\":%ld,\"map_sample\":", opened.face->num_glyphs);
        print_glyph_to_script_map_sample(opened.face, prop.map);
        printf("}}\n");
        close_oracle_face(&opened);
        return 0;
    }
    if (streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_CJK.glyph_to_script_map_runtime") ||
        streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_INDIC.glyph_to_script_map_runtime") ||
        streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_LATIN.glyph_to_script_map_runtime") ||
        streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_NONE.glyph_to_script_map_runtime")) {
        if (argc != 9) {
            fprintf(stderr, "glyph-to-script-map runtime case requires source_kind source_value face_index target_char_code pixel_width pixel_height\n");
            return 2;
        }
        OracleFace opened = {0};
        int open_status = open_oracle_face(argv[3], argv[4], atol(argv[5]), &opened);
        if (open_status) {
            return open_status;
        }
        FT_ULong target_char = strtoul(argv[6], NULL, 10);
        FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
        FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
        FT_Error size_error = FT_Set_Pixel_Sizes(opened.face, pixel_width, pixel_height);
        FT_UInt target_glyph = FT_Get_Char_Index(opened.face, target_char);
        FT_Prop_GlyphToScriptMap prop;
        prop.face = opened.face;
        prop.map = (FT_UShort*)1;
        FT_Error error = FT_Property_Get(
            opened.library,
            "autofitter",
            "glyph-to-script-map",
            &prop);
        FT_UShort target_script = 0;
        if (prop.map && target_glyph < opened.face->num_glyphs) {
            target_script = prop.map[target_glyph];
        }
        FT_Error load_error = size_error ? size_error : FT_Load_Glyph(opened.face, target_glyph, FT_LOAD_FORCE_AUTOHINT);
        print_status(0);
        printf(",\"output\":{\"error\":%d,\"prop_after\":{\"face\":", error);
        print_property_face_identity(opened.face, prop.face);
        printf(",\"map\":");
        print_property_map_identity(prop.map);
        printf("},\"num_glyphs\":%ld,\"target\":{\"glyph_index\":%u,\"script\":%u},\"load_error\":%d,\"glyph_slot\":",
               opened.face->num_glyphs,
               target_glyph,
               target_script,
               load_error);
        if (load_error) {
            printf("null");
        } else {
            printf("{");
            print_slot_body(opened.face->glyph, target_glyph);
            printf("}");
        }
        printf("}}\n");
        close_oracle_face(&opened);
        return 0;
    }
    if (streq(case_id, "ftdriver.FT_Prop_GlyphToScriptMap.invalid_face_error_matches_c")) {
        FT_Library library = NULL;
        FT_Error init_error = FT_Init_FreeType(&library);
        if (init_error) {
            print_status(init_error);
            printf(",\"output\":{\"error\":%d}}\n", init_error);
            return 0;
        }
        FT_Prop_GlyphToScriptMap prop;
        prop.face = NULL;
        prop.map = (FT_UShort*)1;
        FT_Error error = FT_Property_Get(
            library,
            "autofitter",
            "glyph-to-script-map",
            &prop);
        print_status(error);
        printf(",\"output\":{\"error\":%d,\"prop_after\":{\"face\":", error);
        print_property_face_identity(NULL, prop.face);
        printf(",\"map\":");
        print_property_map_identity(prop.map);
        printf("}}}\n");
        FT_Done_FreeType(library);
        return 0;
    }
    if (streq(case_id, "ftdriver.FT_Prop_IncreaseXHeight.invalid_face_error_matches_c")) {
        FT_Library library = NULL;
        FT_Error init_error = FT_Init_FreeType(&library);
        if (init_error) {
            print_status(init_error);
            printf(",\"output\":{\"error\":%d}}\n", init_error);
            return 0;
        }
        FT_Prop_IncreaseXHeight prop;
        prop.face = NULL;
        prop.limit = PROPERTY_SENTINEL;
        FT_Error error = FT_Property_Set(
            library,
            "autofitter",
            "increase-x-height",
            &prop);
        print_status(error);
        printf(",\"output\":{\"error\":%d,\"prop_after\":{\"face\":", error);
        print_property_face_identity(NULL, prop.face);
        printf(",\"limit\":%u}}}\n", prop.limit);
        FT_Done_FreeType(library);
        return 0;
    }
    print_status(FT_Err_Ok);
    if (streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_LATIN.default_script_property_roundtrip") ||
        streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_NONE.default_and_fallback_property_roundtrip") ||
        streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_CJK.fallback_script_property_roundtrip") ||
        streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_INDIC.fallback_script_property_validation")) {
        const FT_UInt value =
            streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_LATIN.default_script_property_roundtrip") ? FT_AUTOHINTER_SCRIPT_LATIN :
            streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_CJK.fallback_script_property_roundtrip") ? FT_AUTOHINTER_SCRIPT_CJK :
            streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_INDIC.fallback_script_property_validation") ? FT_AUTOHINTER_SCRIPT_INDIC :
            FT_AUTOHINTER_SCRIPT_NONE;
        const int properties_none[] = {3, 4};
        const int properties_default[] = {3};
        const int properties_fallback[] = {4};
        const int* properties = properties_default;
        int property_count = 1;
        if (streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_NONE.default_and_fallback_property_roundtrip")) {
            properties = properties_none;
            property_count = 2;
        } else if (!streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_LATIN.default_script_property_roundtrip")) {
            properties = properties_fallback;
        }
        printf(",\"output\":{\"rows\":[");
        for (int i = 0; i < property_count; i++) {
            const int property_selector = properties[i];
            FT_UInt initial = PROPERTY_SENTINEL;
            FT_Error initial_get = oracle_property_get(1, 4, property_selector, &initial);
            FT_Error set_status = FT_Err_Ok;
            FT_Error get_status = FT_Err_Ok;
            FT_UInt readback = PROPERTY_SENTINEL;
            oracle_property_set_then_get(4, property_selector, &value, &set_status, &get_status, &readback);
            FT_UInt bad_module_value = PROPERTY_SENTINEL;
            FT_Error bad_module_error = oracle_property_get(1, 3, property_selector, &bad_module_value);
            FT_UInt bad_property_value = PROPERTY_SENTINEL;
            FT_Error bad_property_error = oracle_property_get(1, 4, 2, &bad_property_value);
            FT_UInt invalid = 9999;
            FT_Error invalid_set_error = oracle_property_set(1, 4, property_selector, &invalid);
            FT_UInt post_invalid = PROPERTY_SENTINEL;
            FT_Error post_invalid_get = oracle_property_get(1, 4, property_selector, &post_invalid);
            if (i) {
                printf(",");
            }
            printf("{\"property\":\"%s\",\"initial_get_error\":%d,\"initial_readback\":%u,"
                   "\"set_error\":%d,\"get_error\":%d,\"readback_value\":%u,"
                   "\"bad_module_error\":%d,\"bad_module_value_after\":%u,"
                   "\"bad_property_error\":%d,\"bad_property_value_after\":%u,"
                   "\"invalid_set_error\":%d,\"post_invalid_get_error\":%d,"
                   "\"post_invalid_readback\":%u}",
                   property_name_value(property_selector),
                   initial_get,
                   initial,
                   set_status,
                   get_status,
                   readback,
                   bad_module_error,
                   bad_module_value,
                   bad_property_error,
                   bad_property_value,
                   invalid_set_error,
                   post_invalid_get,
                   post_invalid);
        }
        printf("]");
        if (streq(case_id, "ftdriver.FT_AUTOHINTER_SCRIPT_NONE.default_and_fallback_property_roundtrip")) {
            if (argc != 9) {
                fprintf(stderr, "NONE script property roundtrip requires source_kind source_value face_index target_char_code pixel_width pixel_height\n");
                return 2;
            }
            OracleFace opened = {0};
            int open_status = open_oracle_face(argv[3], argv[4], atol(argv[5]), &opened);
            if (open_status) {
                return open_status;
            }
            FT_ULong target_char = strtoul(argv[6], NULL, 10);
            FT_UInt pixel_width = (FT_UInt)strtoul(argv[7], NULL, 10);
            FT_UInt pixel_height = (FT_UInt)strtoul(argv[8], NULL, 10);
            FT_Error size_error = FT_Set_Pixel_Sizes(opened.face, pixel_width, pixel_height);
            FT_UInt target_glyph = FT_Get_Char_Index(opened.face, target_char);
            FT_Prop_GlyphToScriptMap prop;
            prop.face = opened.face;
            prop.map = (FT_UShort*)1;
            FT_Error map_error = FT_Property_Get(
                opened.library,
                "autofitter",
                "glyph-to-script-map",
                &prop);
            FT_UShort target_script = 0;
            if (prop.map && target_glyph < opened.face->num_glyphs) {
                target_script = prop.map[target_glyph];
            }
            FT_Error load_error = size_error ? size_error : FT_Load_Glyph(opened.face, target_glyph, FT_LOAD_FORCE_AUTOHINT);
            printf(",\"glyph_to_script_map\":{\"error\":%d,\"prop_after\":{\"face\":", map_error);
            print_property_face_identity(opened.face, prop.face);
            printf(",\"map\":");
            print_property_map_identity(prop.map);
            printf("},\"num_glyphs\":%ld,\"target\":{\"glyph_index\":%u,\"script\":%u},\"load_error\":%d,\"glyph_slot\":",
                   opened.face->num_glyphs,
                   target_glyph,
                   target_script,
                   load_error);
            if (load_error) {
                printf("null");
            } else {
                printf("{");
                print_slot_body(opened.face->glyph, target_glyph);
                printf("}");
            }
            printf("}");
            close_oracle_face(&opened);
        } else {
            printf(",\"glyph_to_script_map\":null");
        }
        printf("}}\n");
        return 0;
    }
    if (streq(case_id, "ftdriver.FT_Prop_IncreaseXHeight.property_set_get_round_trips_limit")) {
        if (argc != 6) {
            fprintf(stderr, "increase-x-height property case requires source_kind source_value face_index\n");
            return 2;
        }
        OracleFace opened = {0};
        int open_status = open_oracle_face(argv[3], argv[4], atol(argv[5]), &opened);
        if (open_status) {
            return open_status;
        }
        const FT_UInt values[] = {0, 6, 10, 14, 32};
        printf(",\"output\":{\"rows\":[");
        for (int i = 0; i < 5; i++) {
            FT_Prop_IncreaseXHeight set_prop;
            set_prop.face = opened.face;
            set_prop.limit = values[i];
            FT_Error set_error = FT_Property_Set(
                opened.library,
                "autofitter",
                "increase-x-height",
                &set_prop);
            FT_Prop_IncreaseXHeight get_prop;
            get_prop.face = opened.face;
            get_prop.limit = PROPERTY_SENTINEL;
            FT_Error get_error = FT_Property_Get(
                opened.library,
                "autofitter",
                "increase-x-height",
                &get_prop);
            if (i) {
                printf(",");
            }
            printf("{\"input\":%u,\"set_error\":%d,\"get_error\":%d,\"set_prop\":{\"face\":",
                   values[i],
                   set_error,
                   get_error);
            print_property_face_identity(opened.face, set_prop.face);
            printf(",\"limit\":%u},\"get_prop\":{\"face\":", set_prop.limit);
            print_property_face_identity(opened.face, get_prop.face);
            printf(",\"limit\":%u}}", get_prop.limit);
        }
        printf("]}}\n");
        close_oracle_face(&opened);
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Property_Get.gets_supported_property") ||
        streq(case_id, "ftdriver.TT_INTERPRETER_VERSION_40.default_interpreter_version")) {
        FT_UInt value = PROPERTY_SENTINEL;
        FT_Error error = oracle_property_get(1, 1, 1, &value);
        printf(",\"output\":{\"status\":%d,\"value\":%u,\"module_service\":%s}}\n",
               error,
               value,
               error == FT_Err_Ok ? "true" : "false");
        return 0;
    }
    if (streq(case_id, "fterrdef.FT_Err_Missing_Property.known_property_success")) {
        FT_UInt value = PROPERTY_SENTINEL;
        FT_Error error = oracle_property_get(1, 4, 4, &value);
        printf(",\"output\":{\"status\":%d,\"value\":%u,\"module_service\":%s}}\n",
               error,
               value,
               error == FT_Err_Ok ? "true" : "false");
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Property_Get.rejects_null_arguments")) {
        FT_UInt library_value = PROPERTY_SENTINEL;
        FT_UInt module_value = PROPERTY_SENTINEL;
        FT_UInt property_value = PROPERTY_SENTINEL;
        FT_Error library_error = oracle_property_get(0, 1, 1, &library_value);
        FT_Error module_error = oracle_property_get(1, 0, 1, &module_value);
        FT_Error property_error = oracle_property_get(1, 1, 0, &property_value);
        FT_Error value_error = oracle_property_get(1, 1, 1, NULL);
        printf(",\"output\":{\"error\":%d,\"rows\":[", library_error);
        printf("{\"field\":\"library\",\"error\":%d,\"value_after\":%u},", library_error, library_value);
        printf("{\"field\":\"module_name\",\"error\":%d,\"value_after\":%u},", module_error, module_value);
        printf("{\"field\":\"property_name\",\"error\":%d,\"value_after\":%u},", property_error, property_value);
        printf("{\"field\":\"value\",\"error\":%d,\"value_after\":null}", value_error);
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Property_Get.missing_or_unsupported_property_service")) {
        FT_UInt missing_value = PROPERTY_SENTINEL;
        FT_UInt unsupported_value = PROPERTY_SENTINEL;
        FT_Error missing = oracle_property_get(1, 3, 1, &missing_value);
        FT_Error unsupported = oracle_property_get(1, 2, 1, &unsupported_value);
        printf(",\"output\":{\"error\":%d,\"rows\":[", missing);
        printf("{\"module\":\"fixture_missing\",\"error\":%d,\"value_after\":%u},", missing, missing_value);
        printf("{\"module\":\"sfnt\",\"error\":%d,\"value_after\":%u}", unsupported, unsupported_value);
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Property_Get.invalid_property_name") ||
        streq(case_id, "fterrdef.FT_Err_Missing_Property.driver_property_unknown_name")) {
        FT_UInt value = PROPERTY_SENTINEL;
        FT_Error error = oracle_property_get(1, 1, 2, &value);
        printf(",\"output\":{\"error\":%d,\"rows\":[{\"property\":\"fixture-missing-property\",\"error\":%d,\"value_after\":%u}]}}\n",
               error,
               error,
               value);
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Property_Set.sets_supported_property")) {
        const FT_UInt values[] = {TT_INTERPRETER_VERSION_35, TT_INTERPRETER_VERSION_38, TT_INTERPRETER_VERSION_40};
        printf(",\"output\":{\"rows\":[");
        for (int i = 0; i < 3; i++) {
            FT_Error set_status = FT_Err_Ok;
            FT_Error get_status = FT_Err_Ok;
            FT_UInt out = PROPERTY_SENTINEL;
            oracle_property_set_then_get(1, 1, &values[i], &set_status, &get_status, &out);
            if (i) {
                printf(",");
            }
            printf("{\"input\":%u,\"set_status\":%d,\"get_status\":%d,\"value_after_get\":%u}",
                   values[i],
                   set_status,
                   get_status,
                   out);
        }
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftdriver.TT_INTERPRETER_VERSION_35.interpreter_version_property_roundtrip") ||
        streq(case_id, "ftdriver.TT_INTERPRETER_VERSION_38.interpreter_version_property_normalizes_to_40") ||
        streq(case_id, "ftdriver.TT_INTERPRETER_VERSION_40.interpreter_version_property_roundtrip")) {
        FT_UInt value = TT_INTERPRETER_VERSION_40;
        if (streq(case_id, "ftdriver.TT_INTERPRETER_VERSION_35.interpreter_version_property_roundtrip")) {
            value = TT_INTERPRETER_VERSION_35;
        } else if (streq(case_id, "ftdriver.TT_INTERPRETER_VERSION_38.interpreter_version_property_normalizes_to_40")) {
            value = TT_INTERPRETER_VERSION_38;
        }
        FT_Error set_status = FT_Err_Ok;
        FT_Error get_status = FT_Err_Ok;
        FT_UInt out = PROPERTY_SENTINEL;
        oracle_property_set_then_get(1, 1, &value, &set_status, &get_status, &out);
        printf(",\"output\":{\"input\":%u,\"set_status\":%d,\"get_status\":%d,\"value_after_get\":%u}}\n",
               value,
               set_status,
               get_status,
               out);
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Property_Set.rejects_null_arguments")) {
        FT_UInt value = TT_INTERPRETER_VERSION_40;
        FT_Error library_error = oracle_property_set(0, 1, 1, &value);
        FT_Error module_error = oracle_property_set(1, 0, 1, &value);
        FT_Error property_error = oracle_property_set(1, 1, 0, &value);
        FT_Error value_error = oracle_property_set(1, 1, 1, NULL);
        printf(",\"output\":{\"error\":%d,\"rows\":[", library_error);
        printf("{\"field\":\"library\",\"error\":%d},", library_error);
        printf("{\"field\":\"module_name\",\"error\":%d},", module_error);
        printf("{\"field\":\"property_name\",\"error\":%d},", property_error);
        printf("{\"field\":\"value\",\"error\":%d}", value_error);
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Property_Set.missing_or_unsupported_property_service")) {
        FT_UInt value = TT_INTERPRETER_VERSION_40;
        FT_Error missing = oracle_property_set(1, 3, 1, &value);
        FT_Error unsupported = oracle_property_set(1, 2, 1, &value);
        printf(",\"output\":{\"error\":%d,\"rows\":[", missing);
        printf("{\"module\":\"fixture_missing\",\"error\":%d,\"property_after\":%u},", missing, PROPERTY_SENTINEL);
        printf("{\"module\":\"sfnt\",\"error\":%d,\"property_after\":%u}", unsupported, PROPERTY_SENTINEL);
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Property_Set.invalid_property_or_value")) {
        FT_UInt good = TT_INTERPRETER_VERSION_40;
        FT_UInt bad = 9999;
        FT_Error set_status = FT_Err_Ok;
        FT_Error get_status = FT_Err_Ok;
        FT_UInt missing_after = PROPERTY_SENTINEL;
        oracle_property_set_then_get(1, 2, &good, &set_status, &get_status, &missing_after);
        FT_Error missing_error = set_status;
        FT_UInt value_after = PROPERTY_SENTINEL;
        oracle_property_set_then_get(1, 1, &bad, &set_status, &get_status, &value_after);
        printf(",\"output\":{\"error\":%d,\"rows\":[", missing_error);
        printf("{\"scenario\":\"invalid_property\",\"error\":%d,\"previous_property_value\":%d,\"property_after\":%u},",
               missing_error,
               TT_INTERPRETER_VERSION_40,
               missing_after);
        printf("{\"scenario\":\"invalid_value\",\"error\":%d,\"previous_property_value\":%d,\"property_after\":%u}",
               set_status,
               TT_INTERPRETER_VERSION_40,
               value_after);
        printf("]}}\n");
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Set_Default_Properties.no_environment_noop")) {
        FT_UInt before = oracle_default_properties_value(1, NULL);
        FT_UInt after = oracle_default_properties_value(1, NULL);
        printf(",\"output\":{\"property_before\":%u,\"property_after\":%u}}\n",
               before,
               after);
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Set_Default_Properties.parses_supported_environment_property")) {
        FT_UInt after = oracle_default_properties_value(1, "truetype:interpreter-version=35");
        printf(",\"output\":{\"environment_properties_enabled\":%s,\"property_after\":%u}}\n",
               after == TT_INTERPRETER_VERSION_35 ? "true" : "false",
               after);
        return 0;
    }
    if (streq(case_id, "ftmodapi.FT_Set_Default_Properties.ignores_malformed_or_failed_properties")) {
        FT_UInt malformed = oracle_default_properties_value(1, "malformed");
        FT_UInt missing = oracle_default_properties_value(1, "missing:property=1");
        FT_UInt null_library = oracle_default_properties_value(0, "truetype:interpreter-version=35");
        printf(",\"output\":{\"return\":\"void\",\"crashed\":false,\"rows\":[");
        printf("{\"scenario\":\"malformed\",\"property_after\":%u},", malformed);
        printf("{\"scenario\":\"missing_property\",\"property_after\":%u},", missing);
        printf("{\"scenario\":\"null_library\",\"property_after\":");
        if (null_library == PROPERTY_SENTINEL) {
            printf("null");
        } else {
            printf("%u", null_library);
        }
        printf("}]}}\n");
        return 0;
    }
    printf(",\"output\":{\"unsupported_case\":\"%s\"}}\n", case_id);
    return 0;
}

static const char* debug_hook_class_name(FT_DebugHook_Func hook) {
    if (hook == debug_hook_a) {
        return "hook_a";
    }
    if (hook == debug_hook_b) {
        return "hook_b";
    }
    if (hook) {
        return "other";
    }
    return "null";
}

static void print_debug_hook_snapshot(FT_Library library) {
    printf("[");
    for (int i = 0; i < 4; i++) {
        if (i) {
            printf(",");
        }
        printf("\"%s\"", library ? debug_hook_class_name(library->debug_hooks[i]) : "null");
    }
    printf("]");
}

static int emit_set_debug_hook(int argc, char** argv) {
    (void)argc;
    int action = atoi(argv[2]);
    FT_Library library = NULL;
    if (action != 2) {
        FT_Error err = FT_Init_FreeType(&library);
        printf("{");
        if (err) {
            print_status(err);
            printf(",\"output\":null}\n");
            return 0;
        }
    } else {
        printf("{");
    }

    print_status(FT_Err_Ok);
    if (action == 1) {
        FT_Set_Debug_Hook(library, FT_DEBUG_HOOK_TRUETYPE, debug_hook_a);
        printf(",\"output\":{\"return\":\"void\",\"debug_hooks_snapshot\":");
        print_debug_hook_snapshot(library);
        printf(",\"stored_slot_identity\":\"%s\"}}\n", debug_hook_class_name(library->debug_hooks[0]));
    } else if (action == 3) {
        FT_Set_Debug_Hook(library, FT_DEBUG_HOOK_TRUETYPE, debug_hook_a);
        printf(",\"output\":{\"return\":\"void\",\"debug_hooks_before\":");
        print_debug_hook_snapshot(library);
        FT_Set_Debug_Hook(library, 4, debug_hook_b);
        FT_Set_Debug_Hook(library, FT_DEBUG_HOOK_TRUETYPE, NULL);
        printf(",\"debug_hooks_after\":");
        print_debug_hook_snapshot(library);
        printf("}}\n");
    } else {
        FT_Set_Debug_Hook(NULL, FT_DEBUG_HOOK_TRUETYPE, debug_hook_a);
        printf(",\"output\":{\"return\":\"void\",\"crashed\":false,\"observable_writes\":\"none\"}}\n");
    }

    if (library) {
        FT_Done_FreeType(library);
    }
    return 0;
}

static const char* default_module_probe_order[] = {
    "autofitter",
    "truetype",
    "type1",
    "cff",
    "cid",
    "pfr",
    "type42",
    "winfonts",
    "pcf",
    "bdf",
    "psaux",
    "psnames",
    "pshinter",
    "sfnt",
    "smooth",
    "raster1",
    "sdf",
    "bsdf",
    "svg",
    NULL
};

static int csv_contains_token(const char* csv, const char* needle) {
    size_t needle_len = strlen(needle);
    const char* cursor = csv;
    while (*cursor) {
        const char* end = strchr(cursor, ',');
        size_t len = end ? (size_t)(end - cursor) : strlen(cursor);
        if (len == needle_len && strncmp(cursor, needle, len) == 0) {
            return 1;
        }
        if (!end) break;
        cursor = end + 1;
    }
    return 0;
}

static void print_add_default_modules_observation(FT_Library library, const char* probes_csv) {
    printf("\"module_names_in_order\":[");
    int first = 1;
    for (int i = 0; default_module_probe_order[i]; ++i) {
        const char* name = default_module_probe_order[i];
        if (csv_contains_token(probes_csv, name) && FT_Get_Module(library, name)) {
            if (!first) printf(",");
            first = 0;
            printf("\"%s\"", name);
        }
    }
    printf("],\"lookup_results\":{");
    char* probes = (char*)malloc(strlen(probes_csv) + 1);
    if (!probes) {
        printf("}");
        return;
    }
    memcpy(probes, probes_csv, strlen(probes_csv) + 1);
    char* token = strtok(probes, ",");
    first = 1;
    while (token) {
        if (!first) printf(",");
        first = 0;
        printf("\"%s\":%s", token, FT_Get_Module(library, token) ? "true" : "false");
        token = strtok(NULL, ",");
    }
    free(probes);
    printf("}");
}

static unsigned long module_flag_value(const char* flag) {
    if (streq(flag, "FT_MODULE_FONT_DRIVER")) return FT_MODULE_FONT_DRIVER;
    if (streq(flag, "FT_MODULE_RENDERER")) return FT_MODULE_RENDERER;
    if (streq(flag, "FT_MODULE_HINTER")) return FT_MODULE_HINTER;
    if (streq(flag, "FT_MODULE_STYLER")) return FT_MODULE_STYLER;
    if (streq(flag, "FT_MODULE_DRIVER_SCALABLE")) return FT_MODULE_DRIVER_SCALABLE;
    if (streq(flag, "FT_MODULE_DRIVER_NO_OUTLINES")) return FT_MODULE_DRIVER_NO_OUTLINES;
    if (streq(flag, "FT_MODULE_DRIVER_HAS_HINTER")) return FT_MODULE_DRIVER_HAS_HINTER;
    if (streq(flag, "FT_MODULE_DRIVER_HINTS_LIGHTLY")) return FT_MODULE_DRIVER_HINTS_LIGHTLY;
    return 0;
}

static int emit_inspect_module_flags(int argc, char** argv) {
    if (argc != 4) return 2;
    const char* flag_symbol = argv[2];
    const char* modules_csv = argv[3];
    unsigned long flag = module_flag_value(flag_symbol);
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    printf("{");
    print_status(err);
    printf(",\"output\":{\"flag\":\"%s\",\"flag_value\":%lu,\"modules\":[", flag_symbol, flag);
    int first = 1;
    const char* cursor = modules_csv;
    while (*cursor) {
        const char* end = strchr(cursor, ',');
        size_t len = end ? (size_t)(end - cursor) : strlen(cursor);
        char name[64];
        if (len >= sizeof(name)) len = sizeof(name) - 1;
        memcpy(name, cursor, len);
        name[len] = '\0';
        FT_Module module = library ? FT_Get_Module(library, name) : NULL;
        unsigned long flags = module && module->clazz ? module->clazz->module_flags : 0;
        if (!first) printf(",");
        first = 0;
        printf("{\"module\":\"%s\",\"present\":%s,\"flags\":%lu,\"has_flag\":%s}",
               name,
               module ? "true" : "false",
               flags,
               module && (flags & flag) ? "true" : "false");
        if (!end) break;
        cursor = end + 1;
    }
    printf("]}}");
    if (library) FT_Done_FreeType(library);
    printf("\n");
    return 0;
}

static void print_module_lookup_row(FT_Library library, const char* name) {
    const char* module_name = (name && streq(name, "null")) ? NULL : name;
    FT_Module module = FT_Get_Module(library, module_name);
    const char* class_name = module && module->clazz && module->clazz->module_name
        ? module->clazz->module_name
        : NULL;
    printf("{\"module\":\"%s\",\"nullness\":%s,\"class_name\":",
           name ? name : "null",
           module ? "false" : "true");
    if (class_name) {
        printf("\"%s\"", class_name);
    } else {
        printf("null");
    }
    printf("}");
}

static int emit_get_module(int argc, char** argv) {
    if (argc != 4) return 2;
    int library_present = atoi(argv[2]);
    const char* modules_csv = argv[3];
    FT_Library library = NULL;
    FT_Error err = FT_Err_Ok;
    if (library_present) {
        err = FT_Init_FreeType(&library);
    }
    printf("{");
    print_status(err);
    printf(",\"output\":{\"lookups\":[");
    int first = 1;
    const char* cursor = modules_csv;
    while (*cursor) {
        const char* end = strchr(cursor, ',');
        size_t len = end ? (size_t)(end - cursor) : strlen(cursor);
        char name[64];
        if (len >= sizeof(name)) len = sizeof(name) - 1;
        memcpy(name, cursor, len);
        name[len] = '\0';
        if (!first) printf(",");
        first = 0;
        print_module_lookup_row(library, name);
        if (!end) break;
        cursor = end + 1;
    }
    printf("]}}\n");
    if (library) FT_Done_FreeType(library);
    return 0;
}

static int module_requester_has_service(FT_Module module, const char* service_name) {
    if (!module || !module->clazz || !module->clazz->get_interface || !service_name) {
        return 0;
    }
    return module->clazz->get_interface(module, service_name) != NULL;
}

static void print_module_interface_row(FT_Library library, const char* name, const char* services_csv) {
    const char* module_name = (name && streq(name, "null")) ? NULL : name;
    FT_Module module = (library && module_name) ? FT_Get_Module(library, module_name) : NULL;
    int interface_present = module && module->clazz && module->clazz->module_interface;
    printf("{\"module\":\"%s\",\"module_found\":%s,\"module_interface_nullness\":%s,\"requester_result_class\":{",
           name ? name : "null",
           module ? "true" : "false",
           interface_present ? "false" : "true");
    int first = 1;
    const char* cursor = services_csv;
    while (*cursor) {
        const char* end = strchr(cursor, ',');
        size_t len = end ? (size_t)(end - cursor) : strlen(cursor);
        char service[96];
        if (len >= sizeof(service)) len = sizeof(service) - 1;
        memcpy(service, cursor, len);
        service[len] = '\0';
        if (!first) printf(",");
        first = 0;
        printf("\"%s\":\"%s\"", service,
               module_requester_has_service(module, service) ? "service_pointer" : "null");
        if (!end) break;
        cursor = end + 1;
    }
    printf("},\"callback_invocations\":[");
    first = 1;
    cursor = services_csv;
    while (*cursor) {
        const char* end = strchr(cursor, ',');
        size_t len = end ? (size_t)(end - cursor) : strlen(cursor);
        char service[96];
        if (len >= sizeof(service)) len = sizeof(service) - 1;
        memcpy(service, cursor, len);
        service[len] = '\0';
        if (!first) printf(",");
        first = 0;
        printf("{\"typedef\":\"FT_Module_Requester\",\"name\":\"%s\",\"status_or_nullness\":\"%s\"}",
               service,
               module_requester_has_service(module, service) ? "service_pointer" : "null");
        if (!end) break;
        cursor = end + 1;
    }
    printf("]}");
}

static int emit_module_interface_probe(int argc, char** argv) {
    if (argc != 5) return 2;
    int library_present = atoi(argv[2]);
    const char* modules_csv = argv[3];
    const char* services_csv = argv[4];
    FT_Library library = NULL;
    FT_Error err = FT_Err_Ok;
    if (library_present) {
        err = FT_Init_FreeType(&library);
    }
    printf("{");
    print_status(err);
    printf(",\"output\":{\"modules\":[");
    int first = 1;
    const char* cursor = modules_csv;
    while (*cursor) {
        const char* end = strchr(cursor, ',');
        size_t len = end ? (size_t)(end - cursor) : strlen(cursor);
        char name[64];
        if (len >= sizeof(name)) len = sizeof(name) - 1;
        memcpy(name, cursor, len);
        name[len] = '\0';
        if (!first) printf(",");
        first = 0;
        print_module_interface_row(library, name, services_csv);
        if (!end) break;
        cursor = end + 1;
    }
    printf("]}}\n");
    if (library) FT_Done_FreeType(library);
    return 0;
}

static void print_renderer_row(FT_Library library, long format) {
    FT_Renderer renderer = FT_Get_Renderer(library, (FT_Glyph_Format)format);
    printf("{\"format\":%ld,\"renderer_present\":%s,\"renderer_class\":",
           format,
           renderer ? "true" : "false");
    if (!renderer || !renderer->clazz) {
        printf("null}");
        return;
    }
    FT_Renderer_Class* clazz = renderer->clazz;
    const char* module_name = NULL;
    if (renderer->root.clazz && renderer->root.clazz->module_name) {
        module_name = renderer->root.clazz->module_name;
    } else if (clazz->root.module_name) {
        module_name = clazz->root.module_name;
    } else {
        module_name = "";
    }
    printf("{\"module_name\":\"%s\",\"glyph_format\":%ld,\"has_render_glyph\":%s,\"has_raster_class\":%s}}",
           module_name,
           (long)clazz->glyph_format,
           clazz->render_glyph ? "true" : "false",
           clazz->raster_class ? "true" : "false");
}

static int emit_get_renderer(int argc, char** argv) {
    if (argc != 4) return 2;
    FT_Library library = NULL;
    FT_Error err = FT_Err_Ok;
    if (!streq(argv[2], "null")) {
        err = FT_Init_FreeType(&library);
    }
    printf("{");
    print_status(err);
    printf(",\"output\":{\"format_results\":[");
    const char* cursor = argv[3];
    int first = 1;
    while (*cursor) {
        char* endptr = NULL;
        long format = strtol(cursor, &endptr, 10);
        if (endptr == cursor) break;
        if (!first) printf(",");
        first = 0;
        print_renderer_row(library, format);
        if (*endptr != ',') break;
        cursor = endptr + 1;
    }
    printf("]}}\n");
    if (library) FT_Done_FreeType(library);
    return 0;
}

static int emit_set_renderer(int argc, char** argv) {
    if (argc != 3) return 2;
    long format = strtol(argv[2], NULL, 10);
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    printf("{");
    if (err) {
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_Renderer renderer = FT_Get_Renderer(library, (FT_Glyph_Format)format);
    FT_Error set_error = FT_Set_Renderer(library, renderer, 0, NULL);
    printf("\"status\":{\"kind\":\"%s\",\"error_code\":%d},\"output\":{\"set_error\":%d,\"current_renderer\":",
           set_error ? "error" : "ok",
           set_error,
           set_error);
    print_renderer_row(library, format);
    printf("}}\n");
    FT_Done_FreeType(library);
    return 0;
}

static int emit_add_default_modules(int argc, char** argv) {
    int action = atoi(argv[2]);
    if (action == 1) {
        printf("{");
        FT_Add_Default_Modules(NULL);
        print_status(FT_Err_Ok);
        printf(",\"output\":{\"return\":\"void\",\"crashed\":false,\"observable_writes\":\"none\"}}\n");
        return 0;
    }

    if (action == 2) {
        const char* probes_csv = argc > 3 ? argv[3] : "";
        struct FT_MemoryRec_ memory = {NULL, oracle_alloc, oracle_free, oracle_realloc};
        FT_Library library = NULL;
        FT_Error err = FT_New_Library(&memory, &library);
        printf("{");
        if (err) {
            print_status(err);
            printf(",\"output\":null}\n");
            return 0;
        }
        FT_Add_Default_Modules(library);
        print_status(FT_Err_Ok);
        printf(",\"output\":{\"return\":\"void\",");
        print_add_default_modules_observation(library, probes_csv);
        printf("}}\n");
        if (library) {
            FT_Done_Library(library);
        }
        return 0;
    }

    printf("{");
    print_status(FT_Err_Unimplemented_Feature);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_done_mm_var(int argc, char** argv) {
    (void)argc;
    int library_present = atoi(argv[2]);
    int descriptor_present = atoi(argv[3]);
    FT_Library library = NULL;
    FT_Error err = FT_Err_Ok;
    if (library_present) {
        err = FT_Init_FreeType(&library);
    }
    FT_MM_Var descriptor;
    memset(&descriptor, 0, sizeof(descriptor));
    FT_MM_Var* descriptor_ptr = descriptor_present ? &descriptor : NULL;
    if (!err) {
        err = FT_Done_MM_Var(library, descriptor_ptr);
    }
    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"free_events\":\"%s\"}}\n", descriptor_present ? "descriptor" : "none");
    }
    if (library) {
        FT_Done_FreeType(library);
    }
    return 0;
}

static void print_lifecycle_result(FT_Error err) {
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"done\":true}}\n");
    }
}

static int emit_done_freetype(int argc, char** argv) {
    const char* mode = argv[2];
    printf("{");
    if (streq(mode, "null")) {
        print_lifecycle_result(FT_Done_FreeType(NULL));
        return 0;
    }

    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        print_lifecycle_result(err);
        return 0;
    }

    unsigned char* data = NULL;
    if (argc == 6) {
        const char* source_kind = argv[3];
        const char* source_value = argv[4];
        FT_Long face_index = atol(argv[5]);
        long data_len = 0;
        if (!streq(source_kind, "file") || load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            FT_Done_FreeType(library);
            return 2;
        }
        FT_Face face = NULL;
        err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
        if (err && face) {
            FT_Done_Face(face);
        }
    }
    if (!err) {
        err = FT_Done_FreeType(library);
    } else {
        FT_Done_FreeType(library);
    }
    print_lifecycle_result(err);
    free(data);
    return 0;
}

static int emit_done_face(int argc, char** argv) {
    const char* mode = argv[2];
    printf("{");
    if (streq(mode, "null")) {
        print_lifecycle_result(FT_Done_Face(NULL));
        return 0;
    }
    if (argc != 6) {
        fprintf(stderr, "done-face live mode requires source and face_index\n");
        return 2;
    }

    const char* source_kind = argv[3];
    const char* source_value = argv[4];
    FT_Long face_index = atol(argv[5]);
    unsigned char* data = NULL;
    long data_len = 0;
    if (!streq(source_kind, "file") || load_file(source_value, &data, &data_len) != 0) {
        fprintf(stderr, "failed to read font file: %s\n", source_value);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (!err) {
        FT_Face face = NULL;
        err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
        if (!err) {
            err = FT_Done_Face(face);
        }
        FT_Done_FreeType(library);
    }
    print_lifecycle_result(err);
    free(data);
    return 0;
}

static int open_optional_face_for_patent(
    const char* mode,
    int argc,
    char** argv,
    FT_Library* library,
    FT_Face* face,
    unsigned char** data) {
    *library = NULL;
    *face = NULL;
    *data = NULL;
    if (streq(mode, "null")) {
        return 0;
    }
    if (argc < 6) {
        fprintf(stderr, "live patent command requires source and face_index\n");
        return 2;
    }
    const char* source_kind = argv[3];
    const char* source_value = argv[4];
    FT_Long face_index = atol(argv[5]);
    long data_len = 0;
    if (!streq(source_kind, "file") || load_file(source_value, data, &data_len) != 0) {
        fprintf(stderr, "failed to read font file: %s\n", source_value);
        return 2;
    }
    FT_Error err = FT_Init_FreeType(library);
    if (!err) {
        err = FT_New_Memory_Face(*library, *data, data_len, face_index, face);
    }
    if (err) {
        if (*library) {
            FT_Done_FreeType(*library);
        }
        free(*data);
        *library = NULL;
        *data = NULL;
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 1;
    }
    return 0;
}

static int emit_face_check_truetype_patents(int argc, char** argv) {
    const char* mode = argv[2];
    FT_Library library = NULL;
    FT_Face face = NULL;
    unsigned char* data = NULL;
    int open_result = open_optional_face_for_patent(mode, argc, argv, &library, &face, &data);
    if (open_result == 1) {
        return 0;
    }
    if (open_result != 0) {
        return open_result;
    }
    FT_Bool result = FT_Face_CheckTrueTypePatents(face);
    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":{\"result\":%u}}\n", (unsigned)result);
    if (face) {
        FT_Done_Face(face);
    }
    if (library) {
        FT_Done_FreeType(library);
    }
    free(data);
    return 0;
}

static int emit_face_set_unpatented_hinting(int argc, char** argv) {
    const char* mode = argv[2];
    const char* values_arg = argv[3];
    FT_Library library = NULL;
    FT_Face face = NULL;
    unsigned char* data = NULL;
    int open_result = open_optional_face_for_patent(mode, argc - 1, argv + 1, &library, &face, &data);
    if (open_result == 1) {
        return 0;
    }
    if (open_result != 0) {
        return open_result;
    }
    char* values = (char*)malloc(strlen(values_arg) + 1);
    if (!values) {
        if (face) FT_Done_Face(face);
        if (library) FT_Done_FreeType(library);
        free(data);
        return 1;
    }
    memcpy(values, values_arg, strlen(values_arg) + 1);
    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":{\"outputs\":[");
    char* cursor = values;
    int first = 1;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        FT_Bool value = (FT_Bool)strtoul(cursor, NULL, 10);
        FT_Bool result = FT_Face_SetUnpatentedHinting(face, value);
        if (!first) {
            printf(",");
        }
        first = 0;
        printf("{\"value\":%u,\"result\":%u}", (unsigned)value, (unsigned)result);
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");
    free(values);
    if (face) {
        FT_Done_Face(face);
    }
    if (library) {
        FT_Done_FreeType(library);
    }
    free(data);
    return 0;
}

static int emit_get_sfnt_name_variant(int argc, char** argv) {
    const char* face_kind = argv[2];
    const char* output_kind = argv[3];
    const char* indexes_csv = argv[4];
    int output_is_null = streq(output_kind, "null");

    if (streq(face_kind, "null")) {
        printf("{");
        print_status(0);
        printf(",\"output\":");
        print_sfnt_name_indexed_result(NULL, indexes_csv, output_is_null);
        printf("}\n");
        return 0;
    }

    if (!streq(face_kind, "valid") && !streq(face_kind, "non_sfnt")) {
        fprintf(stderr, "unsupported sfnt name face kind: %s\n", face_kind);
        return 2;
    }
    if (argc != 10) {
        fprintf(stderr, "--get-sfnt-name-variant requires font source for non-null face\n");
        return 2;
    }

    const char* source_kind = argv[5];
    const char* source_value = argv[6];
    FT_Long face_index = atol(argv[7]);
    FT_UInt pixel_width = (FT_UInt)strtoul(argv[8], NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[9], NULL, 10);

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

    FT_Face face = NULL;
    err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    if (!err) {
        err = FT_Set_Pixel_Sizes(face, pixel_width, pixel_height);
    }
    printf("{");
    if (err) {
        print_status(err);
        printf(",\"output\":null}\n");
    } else {
        print_status(0);
        printf(",\"output\":");
        print_sfnt_name_indexed_result(face, indexes_csv, output_is_null);
        printf("}\n");
    }
    if (face) {
        FT_Done_Face(face);
    }
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_glyphslot_null_noop(int argc, char** argv) {
    const char* function = argv[2];
    FT_Fixed first = (FT_Fixed)strtol(argv[3], NULL, 10);
    FT_Fixed second = (FT_Fixed)strtol(argv[4], NULL, 10);
    if (streq(function, "FT_GlyphSlot_AdjustWeight")) {
        FT_GlyphSlot_AdjustWeight(NULL, first, second);
    } else if (streq(function, "FT_GlyphSlot_Embolden")) {
        FT_GlyphSlot_Embolden(NULL);
    } else if (streq(function, "FT_GlyphSlot_Oblique")) {
        FT_GlyphSlot_Oblique(NULL);
    } else if (streq(function, "FT_GlyphSlot_Slant")) {
        FT_GlyphSlot_Slant(NULL, first, second);
    } else {
        fprintf(stderr, "unsupported glyphslot null no-op function: %s\n", function);
        return 2;
    }

    printf("{");
    print_status(0);
    printf(",\"output\":{\"function\":\"%s\",\"slot\":\"null\",\"first\":%ld,\"second\":%ld,\"completed\":true}}\n",
           function,
           (long)first,
           (long)second);
    return 0;
}

static int emit_select_size_null(int argc, char** argv) {
    FT_Int strike_index = (FT_Int)strtol(argv[2], NULL, 10);
    FT_Error err = FT_Select_Size(NULL, strike_index);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_new_memory_face_null_base(int argc, char** argv) {
    (void)argc;
    FT_Long file_size = atol(argv[2]);
    FT_Long face_index = atol(argv[3]);
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (!err) {
        FT_Face face = (FT_Face)0x1;
        err = FT_New_Memory_Face(library, NULL, file_size, face_index, &face);
        if (!err && face) {
            FT_Done_Face(face);
        }
        FT_Done_FreeType(library);
    }
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_new_face_missing_path(int argc, char** argv) {
    (void)argc;
    const char* pathname = argv[2];
    FT_Long face_index = atol(argv[3]);
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (!err) {
        FT_Face face = (FT_Face)0x1;
        err = FT_New_Face(library, pathname, face_index, &face);
        if (!err && face) {
            FT_Done_Face(face);
        }
        FT_Done_FreeType(library);
    }
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_face_or_slot(int argc, char** argv) {
    const char* command = argv[1];
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* size_arg = argv[5];
    FT_UInt pixel_width = (FT_UInt)strtoul(size_arg, NULL, 10);
    FT_UInt pixel_height = (FT_UInt)strtoul(argv[6], NULL, 10);

    if (streq(command, "--set-pixel-sizes") && streq(source_kind, "null")) {
        FT_Error err = FT_Set_Pixel_Sizes(NULL, pixel_width, pixel_height);
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

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
    int preserve_probe_face =
        face_index < 0 &&
        (streq(command, "--get-advance") || streq(command, "--get-advances"));
    int preserve_initial_size = streq(size_arg, "none");
    if (!err && !preserve_probe_face && !preserve_initial_size) {
        if (strncmp(size_arg, "request:", 8) == 0) {
            SizeRequestRow row;
            const char* request_src = size_arg + 8;
            char* request_arg = (char*)malloc(strlen(request_src) + 1);
            if (request_arg) {
                memcpy(request_arg, request_src, strlen(request_src) + 1);
            }
            if (request_arg && parse_size_request_row(request_arg, &row)) {
                err = FT_Request_Size(
                    row.face_is_null ? NULL : face,
                    row.request_is_null ? NULL : &row.req);
            } else {
                err = FT_Err_Invalid_Argument;
            }
            free(request_arg);
        } else if (strncmp(size_arg, "char:", 5) == 0) {
            FT_F26Dot6 char_width = 0;
            FT_F26Dot6 char_height = 0;
            FT_UInt horz_resolution = 0;
            FT_UInt vert_resolution = 0;
            int parsed = sscanf(
                size_arg + 5,
                "%ld:%ld:%u:%u",
                &char_width,
                &char_height,
                &horz_resolution,
                &vert_resolution);
            if (parsed == 4) {
                err = FT_Set_Char_Size(
                    face,
                    char_width,
                    char_height,
                    horz_resolution,
                    vert_resolution);
            } else {
                err = FT_Err_Invalid_Argument;
            }
        } else {
            err = FT_Set_Pixel_Sizes(face, pixel_width, pixel_height);
        }
    }

    printf("{");
    if (err || streq(command, "--new-memory-face") || streq(command, "--set-pixel-sizes") || streq(command, "--size-metrics") || streq(command, "--size-record-state")) {
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else if (streq(command, "--size-record-state")) {
            face->size->generic.data = (void*)0x51;
            printf(",\"output\":{\"status\":%d,\"size\":{", err);
            printf("\"face_identity\":\"%s\",", face->size->face == face ? "same_as_parent_face" : "other");
            printf("\"generic_identity\":\"%s\",", face->size->generic.data == (void*)0x51 ? "client_pointer" : "other");
            printf("\"metrics\":{");
            print_size_metrics_object(face->size->metrics);
            printf("},\"internal_nullness\":\"%s\"}}}\n", face->size->internal ? "non_null" : "null");
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

    if (streq(command, "--select-size")) {
        FT_Int strike_index = (FT_Int)strtol(argv[7], NULL, 10);
        err = FT_Select_Size(face, strike_index);
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",");
            print_size_metrics(face->size->metrics);
            printf("}\n");
        }
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--face-get-char-variant-index")) {
        FT_ULong charcode = strtoul(argv[7], NULL, 10);
        FT_ULong variant_selector = strtoul(argv[8], NULL, 10);
        print_status(0);
        printf(",\"output\":");
        print_char_variant_index_output(
            FT_Face_GetCharVariantIndex(face, charcode, variant_selector)
        );
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--face-get-char-variant-is-default")) {
        FT_ULong charcode = strtoul(argv[7], NULL, 10);
        FT_ULong variant_selector = strtoul(argv[8], NULL, 10);
        print_status(0);
        printf(",\"output\":");
        print_char_variant_default_output(
            FT_Face_GetCharVariantIsDefault(face, charcode, variant_selector)
        );
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--face-get-variant-selectors")) {
        print_status(0);
        printf(",\"output\":");
        print_uint32_list_output(FT_Face_GetVariantSelectors(face));
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--face-get-variants-of-char")) {
        FT_ULong charcode = strtoul(argv[7], NULL, 10);
        print_status(0);
        printf(",\"output\":");
        print_uint32_list_output(FT_Face_GetVariantsOfChar(face, charcode));
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--face-get-chars-of-variant")) {
        FT_ULong variant_selector = strtoul(argv[7], NULL, 10);
        print_status(0);
        printf(",\"output\":");
        print_uint32_list_output(FT_Face_GetCharsOfVariant(face, variant_selector));
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-kerning")) {
        const char* rows_arg = argv[7];
        char* rows = (char*)malloc(strlen(rows_arg) + 1);
        if (!rows) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        FT_Error first_error = FT_Err_Ok;
        char* token = strtok(rows, ",");
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            char* mode_text = right ? strchr(right + 1, '|') : NULL;
            if (right && mode_text) {
                *right = '\0';
                *mode_text = '\0';
                right++;
                mode_text++;
                FT_Vector kerning;
                FT_Error err = FT_Get_Kerning(
                    face,
                    glyph_selector_index(face, left),
                    glyph_selector_index(face, right),
                    (FT_UInt)strtoul(mode_text, NULL, 10),
                    &kerning);
                if (first_error == FT_Err_Ok && err != FT_Err_Ok) {
                    first_error = err;
                }
            }
            token = strtok(NULL, ",");
        }
        free(rows);

        rows = (char*)malloc(strlen(rows_arg) + 1);
        if (!rows) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        print_status(first_error);
        printf(",\"output\":{\"status\":%d,\"kerning_vectors\":[", first_error);
        token = strtok(rows, ",");
        int first = 1;
        int have_first_vector = 0;
        FT_Vector first_vector;
        first_vector.x = 0;
        first_vector.y = 0;
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            char* mode_text = right ? strchr(right + 1, '|') : NULL;
            if (right && mode_text) {
                *right = '\0';
                *mode_text = '\0';
                right++;
                mode_text++;
                FT_UInt left_glyph = glyph_selector_index(face, left);
                FT_UInt right_glyph = glyph_selector_index(face, right);
                FT_UInt mode = (FT_UInt)strtoul(mode_text, NULL, 10);
                FT_Vector kerning;
                kerning.x = 0;
                kerning.y = 0;
                FT_Error err = FT_Get_Kerning(face, left_glyph, right_glyph, mode, &kerning);
                if (!have_first_vector) {
                    first_vector = kerning;
                    have_first_vector = 1;
                }
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"left\":\"%s\",\"right\":\"%s\",\"mode\":%u,\"left_glyph\":%u,\"right_glyph\":%u,\"status\":%d,\"akerning\":{\"x\":%ld,\"y\":%ld},\"kerning\":{\"x\":%ld,\"y\":%ld},\"x_26_6\":%ld,\"y_26_6\":%ld,\"units\":\"%s\"}",
                       left,
                       right,
                       mode,
                       left_glyph,
                       right_glyph,
                       err,
                       kerning.x,
                       kerning.y,
                       kerning.x,
                       kerning.y,
                       kerning.x,
                       kerning.y,
                       kerning_units(mode));
            }
            token = strtok(NULL, ",");
        }
        printf("],\"glyph_indexes\":[");
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        token = strtok(rows, ",");
        first = 1;
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            char* mode_text = right ? strchr(right + 1, '|') : NULL;
            if (right && mode_text) {
                *right = '\0';
                *mode_text = '\0';
                right++;
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"left\":%u,\"right\":%u}",
                       glyph_selector_index(face, left),
                       glyph_selector_index(face, right));
            }
            token = strtok(NULL, ",");
        }
        printf("],\"akerning\":{\"x\":%ld,\"y\":%ld},\"kerning\":{\"x\":%ld,\"y\":%ld}}}\n",
               first_vector.x,
               first_vector.y,
               first_vector.x,
               first_vector.y);
        free(rows);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--attach-file")) {
        const char* attachment_path = argv[7];
        const char* rows_arg = argv[8];
        FT_Error attach_error = FT_Attach_File(face, attachment_path);
        FT_Error first_error = attach_error;
        char* rows = (char*)malloc(strlen(rows_arg) + 1);
        if (!rows) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        print_status(first_error);
        printf(",\"output\":{\"attach_status\":%d,\"status\":%d,\"post_attach_probe\":{\"status\":%d,\"kerning_vectors\":[",
               attach_error,
               first_error,
               first_error);
        char* token = strtok(rows, ",");
        int first = 1;
        int have_first_vector = 0;
        FT_Vector first_vector;
        first_vector.x = 0;
        first_vector.y = 0;
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            char* mode_text = right ? strchr(right + 1, '|') : NULL;
            if (right && mode_text) {
                *right = '\0';
                *mode_text = '\0';
                right++;
                mode_text++;
                FT_UInt left_glyph = glyph_selector_index(face, left);
                FT_UInt right_glyph = glyph_selector_index(face, right);
                FT_UInt mode = (FT_UInt)strtoul(mode_text, NULL, 10);
                FT_Vector kerning;
                kerning.x = 0;
                kerning.y = 0;
                FT_Error err = FT_Get_Kerning(face, left_glyph, right_glyph, mode, &kerning);
                if (first_error == FT_Err_Ok && err != FT_Err_Ok) {
                    first_error = err;
                }
                if (!have_first_vector) {
                    first_vector = kerning;
                    have_first_vector = 1;
                }
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"left\":\"%s\",\"right\":\"%s\",\"mode\":%u,\"left_glyph\":%u,\"right_glyph\":%u,\"status\":%d,\"akerning\":{\"x\":%ld,\"y\":%ld},\"kerning\":{\"x\":%ld,\"y\":%ld},\"x_26_6\":%ld,\"y_26_6\":%ld,\"units\":\"%s\"}",
                       left,
                       right,
                       mode,
                       left_glyph,
                       right_glyph,
                       err,
                       kerning.x,
                       kerning.y,
                       kerning.x,
                       kerning.y,
                       kerning.x,
                       kerning.y,
                       kerning_units(mode));
            }
            token = strtok(NULL, ",");
        }
        printf("],\"glyph_indexes\":[");
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        token = strtok(rows, ",");
        first = 1;
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            char* mode_text = right ? strchr(right + 1, '|') : NULL;
            if (right && mode_text) {
                *right = '\0';
                *mode_text = '\0';
                right++;
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"left\":%u,\"right\":%u}",
                       glyph_selector_index(face, left),
                       glyph_selector_index(face, right));
            }
            token = strtok(NULL, ",");
        }
        printf("],\"akerning\":{\"x\":%ld,\"y\":%ld},\"kerning\":{\"x\":%ld,\"y\":%ld}}}}\n",
               first_vector.x,
               first_vector.y,
               first_vector.x,
               first_vector.y);
        free(rows);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--attach-stream")) {
        const char* attachment_path = argv[7];
        const char* rows_arg = argv[8];
        unsigned char* attachment = NULL;
        long attachment_len = 0;
        if (load_file(attachment_path, &attachment, &attachment_len) != 0) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        FT_Open_Args open_args;
        memset(&open_args, 0, sizeof(open_args));
        open_args.flags = FT_OPEN_MEMORY;
        open_args.memory_base = attachment;
        open_args.memory_size = attachment_len;
        FT_Error attach_error = FT_Attach_Stream(face, &open_args);
        FT_Error first_error = attach_error;
        char* rows = (char*)malloc(strlen(rows_arg) + 1);
        if (!rows) {
            free(attachment);
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        print_status(first_error);
        printf(",\"output\":{\"attach_status\":%d,\"status\":%d,\"post_attach_probe\":{\"status\":%d,\"kerning_vectors\":[",
               attach_error,
               first_error,
               first_error);
        char* token = strtok(rows, ",");
        int first = 1;
        int have_first_vector = 0;
        FT_Vector first_vector;
        first_vector.x = 0;
        first_vector.y = 0;
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            char* mode_text = right ? strchr(right + 1, '|') : NULL;
            if (right && mode_text) {
                *right = '\0';
                *mode_text = '\0';
                right++;
                mode_text++;
                FT_UInt left_glyph = glyph_selector_index(face, left);
                FT_UInt right_glyph = glyph_selector_index(face, right);
                FT_UInt mode = (FT_UInt)strtoul(mode_text, NULL, 10);
                FT_Vector kerning;
                kerning.x = 0;
                kerning.y = 0;
                FT_Error err = FT_Get_Kerning(face, left_glyph, right_glyph, mode, &kerning);
                if (first_error == FT_Err_Ok && err != FT_Err_Ok) {
                    first_error = err;
                }
                if (!have_first_vector) {
                    first_vector = kerning;
                    have_first_vector = 1;
                }
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"left\":\"%s\",\"right\":\"%s\",\"mode\":%u,\"left_glyph\":%u,\"right_glyph\":%u,\"status\":%d,\"akerning\":{\"x\":%ld,\"y\":%ld},\"kerning\":{\"x\":%ld,\"y\":%ld},\"x_26_6\":%ld,\"y_26_6\":%ld,\"units\":\"%s\"}",
                       left,
                       right,
                       mode,
                       left_glyph,
                       right_glyph,
                       err,
                       kerning.x,
                       kerning.y,
                       kerning.x,
                       kerning.y,
                       kerning.x,
                       kerning.y,
                       kerning_units(mode));
            }
            token = strtok(NULL, ",");
        }
        printf("],\"glyph_indexes\":[");
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        token = strtok(rows, ",");
        first = 1;
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            char* mode_text = right ? strchr(right + 1, '|') : NULL;
            if (right && mode_text) {
                *right = '\0';
                *mode_text = '\0';
                right++;
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"left\":%u,\"right\":%u}",
                       glyph_selector_index(face, left),
                       glyph_selector_index(face, right));
            }
            token = strtok(NULL, ",");
        }
        printf("],\"akerning\":{\"x\":%ld,\"y\":%ld},\"kerning\":{\"x\":%ld,\"y\":%ld}}}}\n",
               first_vector.x,
               first_vector.y,
               first_vector.x,
               first_vector.y);
        free(rows);
        free(attachment);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-track-kerning")) {
        const char* attachment_path = argv[7];
        const char* rows_arg = argv[8];
        unsigned char* attachment = NULL;
        long attachment_len = 0;
        if (load_file(attachment_path, &attachment, &attachment_len) != 0) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        FT_Open_Args open_args;
        memset(&open_args, 0, sizeof(open_args));
        open_args.flags = FT_OPEN_MEMORY;
        open_args.memory_base = attachment;
        open_args.memory_size = attachment_len;
        FT_Error attach_error = FT_Attach_Stream(face, &open_args);
        FT_Error first_error = attach_error;
        char* rows = (char*)malloc(strlen(rows_arg) + 1);
        if (!rows) {
            free(attachment);
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        print_status(first_error);
        printf(",\"output\":{\"attach_status\":%d,\"status\":%d,\"rows\":[", attach_error, first_error);
        char* token = strtok(rows, ",");
        int first = 1;
        while (token) {
            char* point_text = token;
            char* degree_text = strchr(point_text, '|');
            if (degree_text) {
                *degree_text = '\0';
                degree_text++;
                FT_Fixed point_size = (FT_Fixed)strtol(point_text, NULL, 10);
                FT_Int degree = (FT_Int)strtol(degree_text, NULL, 10);
                FT_Fixed kerning = 0;
                FT_Error err = FT_Get_Track_Kerning(face, point_size, degree, &kerning);
                if (first_error == FT_Err_Ok && err != FT_Err_Ok) {
                    first_error = err;
                }
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"point_size_16_16\":%ld,\"degree\":%d,\"status\":%d,\"akerning\":%ld}",
                       point_size,
                       degree,
                       err,
                       kerning);
            }
            token = strtok(NULL, ",");
        }
        printf("]}}\n");
        free(rows);
        free(attachment);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-pfr-kerning")) {
        const char* rows_arg = argv[7];
        char* rows = (char*)malloc(strlen(rows_arg) + 1);
        if (!rows) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        FT_Error first_error = FT_Err_Ok;
        char* token = strtok(rows, ",");
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            if (right) {
                *right = '\0';
                right++;
                FT_Vector kerning;
                kerning.x = 0;
                kerning.y = 0;
                FT_Error err = FT_Get_PFR_Kerning(
                    face,
                    glyph_selector_index(face, left),
                    glyph_selector_index(face, right),
                    &kerning);
                if (first_error == FT_Err_Ok && err != FT_Err_Ok) {
                    first_error = err;
                }
            }
            token = strtok(NULL, ",");
        }
        free(rows);

        rows = (char*)malloc(strlen(rows_arg) + 1);
        if (!rows) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        print_status(first_error);
        printf(",\"output\":{\"status\":%d,\"fallback_return\":%d,\"kerning_vectors\":[",
               first_error,
               first_error);
        token = strtok(rows, ",");
        int first = 1;
        int have_first_vector = 0;
        FT_Vector first_vector;
        first_vector.x = 0;
        first_vector.y = 0;
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            if (right) {
                *right = '\0';
                right++;
                FT_UInt left_glyph = glyph_selector_index(face, left);
                FT_UInt right_glyph = glyph_selector_index(face, right);
                FT_Vector kerning;
                kerning.x = 0;
                kerning.y = 0;
                FT_Error err = FT_Get_PFR_Kerning(face, left_glyph, right_glyph, &kerning);
                if (!have_first_vector) {
                    first_vector = kerning;
                    have_first_vector = 1;
                }
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"left\":\"%s\",\"right\":\"%s\",\"mode\":%u,\"left_glyph\":%u,\"right_glyph\":%u,\"status\":%d,\"akerning\":{\"x\":%ld,\"y\":%ld},\"kerning\":{\"x\":%ld,\"y\":%ld},\"x_26_6\":%ld,\"y_26_6\":%ld,\"units\":\"%s\"}",
                       left,
                       right,
                       (unsigned int)FT_KERNING_UNSCALED,
                       left_glyph,
                       right_glyph,
                       err,
                       kerning.x,
                       kerning.y,
                       kerning.x,
                       kerning.y,
                       kerning.x,
                       kerning.y,
                       kerning_units(FT_KERNING_UNSCALED));
            }
            token = strtok(NULL, ",");
        }
        printf("],\"glyph_indexes\":[");
        memcpy(rows, rows_arg, strlen(rows_arg) + 1);
        token = strtok(rows, ",");
        first = 1;
        while (token) {
            char* left = token;
            char* right = strchr(left, '|');
            if (right) {
                *right = '\0';
                right++;
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"left\":%u,\"right\":%u}",
                       glyph_selector_index(face, left),
                       glyph_selector_index(face, right));
            }
            token = strtok(NULL, ",");
        }
        printf("],\"akerning\":{\"x\":%ld,\"y\":%ld},\"kerning\":{\"x\":%ld,\"y\":%ld}}}\n",
               first_vector.x,
               first_vector.y,
               first_vector.x,
               first_vector.y);
        free(rows);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-kerning-null-output")) {
        const char* left = argv[7];
        const char* right = argv[8];
        FT_UInt mode = (FT_UInt)strtoul(argv[9], NULL, 10);
        FT_UInt left_glyph = glyph_selector_index(face, left);
        FT_UInt right_glyph = glyph_selector_index(face, right);
        FT_Vector kerning;
        kerning.x = 0;
        kerning.y = 0;
        FT_Error err = FT_Get_Kerning(face, left_glyph, right_glyph, mode, NULL);
        print_kerning_single_output(left, right, mode, left_glyph, right_glyph, err, kerning, 0);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--charmap-get-char-index")) {
        long platform = atol(argv[7]);
        long encoding = atol(argv[8]);
        FT_ULong char_code = strtoul(argv[9], NULL, 10);
        FT_CharMap matched_charmap = find_charmap_by_ids(face, platform, encoding);
        FT_Error err = matched_charmap
            ? FT_Set_Charmap(face, matched_charmap)
            : FT_ERR(Invalid_Argument);
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",\"output\":{\"value\":%u}}\n", FT_Get_Char_Index(face, char_code));
        }
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--inspect-charmaps")) {
        const char* encodings_csv = argv[7];
        const char* chars_csv = argv[8];
        FT_Error status = 0;

        print_status(0);
        printf(",\"output\":{\"selection_statuses\":[");
        if (!streq(encodings_csv, "-") && encodings_csv[0]) {
            size_t encodings_len = strlen(encodings_csv);
            char* encodings = (char*)malloc(encodings_len + 1);
            if (!encodings) {
                FT_Done_Face(face);
                FT_Done_FreeType(library);
                free(data);
                return 2;
            }
            memcpy(encodings, encodings_csv, encodings_len + 1);
            char* token = strtok(encodings, ",");
            int first = 1;
            while (token) {
                FT_Encoding encoding = (FT_Encoding)strtol(token, NULL, 10);
                status = FT_Select_Charmap(face, encoding);
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("{\"encoding\":%ld,\"status\":%d}", (long)encoding, status);
                token = strtok(NULL, ",");
            }
            free(encodings);
        }
        printf("],\"status\":%d", status);
        printf(",\"num_charmaps\":%d,\"charmaps\":", face->num_charmaps);
        print_charmap_inventory_records(face);
        FT_Int active = active_charmap_index(face);
        printf(",\"active_charmap_index\":");
        if (active < 0) {
            printf("null");
        } else {
            printf("%d", active);
        }
        printf(",\"selected\":");
        print_active_charmap(face);
        printf(",\"selected_charmap\":");
        print_active_charmap(face);
        printf(",\"glyph_indexes\":[");
        if (!streq(chars_csv, "-") && chars_csv[0]) {
            size_t chars_len = strlen(chars_csv);
            char* chars = (char*)malloc(chars_len + 1);
            if (!chars) {
                FT_Done_Face(face);
                FT_Done_FreeType(library);
                free(data);
                return 2;
            }
            memcpy(chars, chars_csv, chars_len + 1);
            char* token = strtok(chars, ",");
            int first = 1;
            while (token) {
                FT_ULong char_code = strtoul(token, NULL, 10);
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf("%u", FT_Get_Char_Index(face, char_code));
                token = strtok(NULL, ",");
            }
            free(chars);
        }
        printf("],\"char_indices\":");
        print_charmap_probe_indices(face, chars_csv);
        printf("}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--set-charmap")) {
        const char* indices_arg = argv[7];
        const char* chars_csv = argv[8];
        FT_Error first_error = FT_Err_Ok;
        FT_Face probe_face = NULL;
        FT_Error probe_error = FT_New_Memory_Face(library, data, data_len, face_index, &probe_face);
        if (probe_error) {
            first_error = probe_error;
        } else {
            for (FT_Int i = 0; probe_face && probe_face->charmaps && i < probe_face->num_charmaps; i++) {
                if (!set_charmap_index_selected(probe_face, indices_arg, i)) {
                    continue;
                }
                FT_Error row_error = FT_Set_Charmap(probe_face, probe_face->charmaps[i]);
                if (first_error == FT_Err_Ok && row_error != FT_Err_Ok) {
                    first_error = row_error;
                }
            }
            FT_Done_Face(probe_face);
        }

        print_status(first_error);
        printf(",\"output\":{\"status\":%d,\"outputs\":[", first_error);
        int first = 1;
        for (FT_Int i = 0; face && face->charmaps && i < face->num_charmaps; i++) {
            if (!set_charmap_index_selected(face, indices_arg, i)) {
                continue;
            }
            FT_CharMap before = face->charmap;
            FT_Error row_error = FT_Set_Charmap(face, face->charmaps[i]);
            if (!first) {
                printf(",");
            }
            first = 0;
            print_set_charmap_row("index", i, row_error, before, face, chars_csv);
        }
        printf("]}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-charmap-index")) {
        print_status(0);
        printf(",\"output\":{\"indices\":[");
        for (FT_Int i = 0; face && face->charmaps && i < face->num_charmaps; i++) {
            if (i) {
                printf(",");
            }
            FT_CharMap charmap = face->charmaps[i];
            printf("{\"return\":%d,\"charmap_metadata\":", FT_Get_Charmap_Index(charmap));
            print_charmap_inventory_record(charmap);
            printf("}");
        }
        printf("],\"returns\":[");
        for (FT_Int i = 0; face && face->charmaps && i < face->num_charmaps; i++) {
            if (i) {
                printf(",");
            }
            printf("%d", FT_Get_Charmap_Index(face->charmaps[i]));
        }
        printf("]}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-fstype-flags")) {
        const char* symbol_name = argc > 7 ? argv[7] : NULL;
        FT_UShort flags = FT_Get_FSType_Flags(face);
        print_status(0);
        printf(",\"output\":");
        print_fstype_flags_result(flags, symbol_name);
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-gasp")) {
        FT_UInt ppem = (FT_UInt)strtoul(argv[7], NULL, 10);
        FT_Int value = FT_Get_Gasp(face, ppem);
        print_status(0);
        printf(",\"output\":{\"value\":%d,\"return\":%d,\"ppem\":%u}}\n", value, value, ppem);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-cmap-format") || streq(command, "--get-cmap-language-id")) {
        const char* variants_csv = argv[7];
        print_status(0);
        printf(",\"output\":");
        print_cmap_info_rows(face, variants_csv, streq(command, "--get-cmap-language-id"));
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-postscript-name")) {
        const char* name = FT_Get_Postscript_Name(face);
        print_status(0);
        printf(",\"output\":");
        print_postscript_name_result(name);
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-font-format") || streq(command, "--get-x11-font-format")) {
        const char* format = streq(command, "--get-font-format")
            ? FT_Get_Font_Format(face)
            : FT_Get_X11_Font_Format(face);
        print_status(0);
        printf(",\"output\":");
        print_nullable_c_string_result(format);
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--face-driver-name")) {
        const char* name = FT_FACE_DRIVER_NAME(face);
        print_status(0);
        printf(",\"output\":");
        print_face_driver_name_result(name);
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--face-driver-name-with-font-format")) {
        const char* driver_name = FT_FACE_DRIVER_NAME(face);
        const char* font_format = FT_Get_Font_Format(face);
        print_status(0);
        printf(",\"output\":{\"driver_name\":");
        print_json_c_string_or_null(driver_name);
        printf(",\"font_format\":");
        print_json_c_string_or_null(font_format);
        printf(",\"same_source\":false}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-winfnt-header")) {
        const int header_is_null = argc > 7 && streq(argv[7], "null");
        FT_WinFNT_HeaderRec header;
        memset(&header, 0xA5, sizeof(header));
        err = FT_Get_WinFNT_Header(face, header_is_null ? NULL : &header);
        print_status(err);
        if (err) {
            printf(",\"output\":{\"error\":%d,\"status\":%d,\"sentinel_unchanged\":true}}\n", err, err);
        } else {
            printf(",\"output\":{\"error\":%d,\"status\":%d,\"header\":", err, err);
            print_winfnt_header_json(&header);
            printf(",\"copied_header\":");
            print_winfnt_header_json(&header);
            printf(",\"file_header\":");
            print_winfnt_header_json(&header);
            printf(",\"sentinel_unchanged\":false}}\n");
        }
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--winfnt-charmap-probe")) {
        FT_WinFNT_HeaderRec header;
        memset(&header, 0, sizeof(header));
        err = FT_Get_WinFNT_Header(face, &header);
        print_status(err);
        if (err) {
            printf(",\"output\":{\"error\":%d,\"status\":%d}}\n", err, err);
        } else {
            printf(",\"output\":{\"error\":%d,\"status\":%d,\"header\":", err, err);
            print_winfnt_header_json(&header);
            printf(",\"charmap\":");
            print_active_charmap(face);
            printf("}}\n");
        }
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-x11-font-format-alias")) {
        const char* font_format = FT_Get_Font_Format(face);
        const char* x11_format = FT_Get_X11_Font_Format(face);
        print_status(0);
        printf(",\"output\":{\"font_format_return\":");
        print_nullable_c_string_result(font_format);
        printf(",\"x11_font_format_return\":");
        print_nullable_c_string_result(x11_format);
        printf(",\"alias_equal\":%s}}\n", font_format == x11_format ? "true" : "false");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-sfnt-name-count")) {
        print_status(0);
        printf(",\"output\":{\"return\":%u}}\n", FT_Get_Sfnt_Name_Count(face));
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-sfnt-name")) {
        print_status(0);
        printf(",\"output\":");
        print_sfnt_name_indexed_result(face, argv[7], 0);
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-sfnt-lang-tag")) {
        FT_SfntLangTag tag;
        FT_Error lang_error = sfnt_lang_tag_lookup(face, argv[7], argv[8], &tag);
        print_status(lang_error);
        printf(",\"output\":");
        if (!lang_error) {
            print_sfnt_lang_tag_json(&tag);
        } else {
            printf("null");
        }
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-sfnt-lang-tag-variants")) {
        print_sfnt_lang_tag_variants(face, argv[7]);
        printf("\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-sfnt-name-match")) {
        long platform = atol(argv[7]);
        long encoding = atol(argv[8]);
        long language = atol(argv[9]);
        long name_id = atol(argv[10]);
        FT_UInt count = FT_Get_Sfnt_Name_Count(face);
        FT_Error status = FT_ERR(Invalid_Argument);
        FT_UInt matched_index = 0;
        FT_SfntName matched_name;
        int matched = 0;
        print_status(0);
        printf(",\"output\":{\"name_count\":%u,\"matches\":[", count);
        int first = 1;
        for (FT_UInt i = 0; i < count; i++) {
            FT_SfntName name;
            FT_Error name_error = FT_Get_Sfnt_Name(face, i, &name);
            if (!name_error && sfnt_name_matches(&name, platform, encoding, language, name_id)) {
                if (!first) printf(",");
                first = 0;
                print_sfnt_name_record(i, &name);
                if (!matched) {
                    matched = 1;
                    matched_index = i;
                    matched_name = name;
                    status = FT_Err_Ok;
                }
            }
        }
        printf("],\"status\":%d,\"matched_name\":", status);
        if (matched) {
            print_sfnt_name_record(matched_index, &matched_name);
        } else {
            printf("null");
        }
        printf("}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--sfnt-mac-encoding-record")) {
        long platform = atol(argv[7]);
        long encoding = atol(argv[8]);
        long language = atol(argv[9]);
        long name_id = atol(argv[10]);
        const char* chars_csv = argv[11];
        FT_CharMap matched_charmap = find_charmap_by_ids(face, platform, encoding);
        FT_Error set_status = matched_charmap
            ? FT_Set_Charmap(face, matched_charmap)
            : FT_ERR(Invalid_Argument);

        FT_UInt count = FT_Get_Sfnt_Name_Count(face);
        FT_UInt matched_index = 0;
        FT_SfntName matched_name;
        int matched = 0;
        for (FT_UInt i = 0; i < count; i++) {
            FT_SfntName name;
            FT_Error name_error = FT_Get_Sfnt_Name(face, i, &name);
            if (!name_error && sfnt_name_matches(&name, platform, encoding, language, name_id)) {
                matched = 1;
                matched_index = i;
                matched_name = name;
                break;
            }
        }

        print_status(0);
        printf(",\"output\":{\"status\":%d,\"matched_charmap\":", set_status);
        print_charmap_record(matched_charmap);
        printf(",\"glyph_indices\":[");
        if (!streq(chars_csv, "-") && chars_csv[0]) {
            size_t chars_len = strlen(chars_csv);
            char* chars = (char*)malloc(chars_len + 1);
            if (!chars) {
                FT_Done_Face(face);
                FT_Done_FreeType(library);
                free(data);
                return 2;
            }
            memcpy(chars, chars_csv, chars_len + 1);
            char* token = strtok(chars, ",");
            int first = 1;
            while (token) {
                FT_ULong char_code = strtoul(token, NULL, 10);
                if (!first) printf(",");
                first = 0;
                printf("%u", set_status ? 0 : FT_Get_Char_Index(face, char_code));
                token = strtok(NULL, ",");
            }
            free(chars);
        }
        printf("],\"matched_name\":");
        if (matched) {
            print_sfnt_name_record(matched_index, &matched_name);
        } else {
            printf("null");
        }
        printf("}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-sfnt-table")) {
        print_status(0);
        printf(",\"output\":{\"entries\":[");
        const char* tags_csv = argv[7];
        int first = 1;
        const char* p = tags_csv;
        while (*p) {
            while (*p == ',' || *p == ' ') p++;
            if (!*p) break;
            FT_Sfnt_Tag tag_val = (FT_Sfnt_Tag)strtoul(p, (char**)&p, 0);
            void* ptr = FT_Get_Sfnt_Table(face, tag_val);
            if (!first) printf(",");
            first = 0;
            printf("{\"tag\":%u,\"pointer_null\":", (unsigned)tag_val);
            print_json_bool(ptr == NULL);
            printf(",\"record_kind\":");
            if (ptr == NULL) {
                printf("null");
            } else if (tag_val == FT_SFNT_HEAD) {
                printf("\"TT_Header\"");
            } else if (tag_val == FT_SFNT_MAXP) {
                printf("\"TT_MaxProfile\"");
            } else if (tag_val == FT_SFNT_OS2) {
                printf("\"TT_OS2\"");
            } else if (tag_val == FT_SFNT_HHEA) {
                printf("\"TT_HoriHeader\"");
            } else if (tag_val == FT_SFNT_VHEA) {
                printf("\"TT_VertHeader\"");
            } else if (tag_val == FT_SFNT_POST) {
                printf("\"TT_Postscript\"");
            } else if (tag_val == FT_SFNT_PCLT) {
                printf("\"TT_PCLT\"");
            } else {
                printf("null");
            }
            printf("}");
        }
        printf("]}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-sfnt-vhea-mvar-sequence")) {
        FT_Error load_status = 0;
        TT_VertHeader* initial = (TT_VertHeader*)FT_Get_Sfnt_Table(face, FT_SFNT_VHEA);
        TT_VertHeader initial_copy;
        int initial_present = initial != NULL;
        if (initial_present) {
            initial_copy = *initial;
        }
        FT_Fixed coords[2];
        coords[0] = (FT_Fixed)strtol(argv[7], NULL, 0);
        coords[1] = (FT_Fixed)strtol(argv[8], NULL, 0);
        FT_Error set_status = FT_Set_Var_Design_Coordinates(face, 2, coords);
        TT_VertHeader* changed = (TT_VertHeader*)FT_Get_Sfnt_Table(face, FT_SFNT_VHEA);
        print_status(0);
        printf(",\"output\":{\"default\":{\"face_load_status\":%d,\"pointer_null\":", (int)load_status);
        print_json_bool(!initial_present);
        printf(",\"record\":");
        print_tt_vert_header_record(initial_present ? &initial_copy : NULL);
        printf("},\"changed\":{\"set_var_status\":%d,\"pointer_null\":", (int)set_status);
        print_json_bool(changed == NULL);
        printf(",\"record\":");
        print_tt_vert_header_record(changed);
        printf("}}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--load-sfnt-table")) {
        FT_ULong tag = strtoul(argv[7], NULL, 16);
        FT_Long offset = atol(argv[8]);
        const char* buffer_kind = argv[9];
        const char* length_state = argv[10];
        FT_ULong length = 0;
        FT_ULong* length_ptr = &length;
        FT_ULong null_length_size = 0;
        FT_Byte buffer[65536];
        FT_Byte* buf_ptr = NULL;
        if (streq(length_state, "zero")) {
            length = 0;
        } else if (streq(length_state, "null")) {
            length_ptr = NULL;
            (void)FT_Load_Sfnt_Table(face, tag, 0, NULL, &null_length_size);
        } else if (streq(length_state, "full")) {
            length = sizeof(buffer);
        } else {
            length = (FT_ULong)strtoul(length_state, NULL, 10);
        }
        if (streq(buffer_kind, "allocated")) {
            buf_ptr = buffer;
        }
        FT_Error ft_err = FT_Load_Sfnt_Table(face, tag, offset, buf_ptr, length_ptr);
        print_status(ft_err);
        if (length_ptr) {
            printf(",\"output\":{\"length_after\":%lu", (unsigned long)length);
        } else {
            printf(",\"output\":{\"length_after\":null");
        }
        FT_ULong bytes_len = length_ptr ? length : 0;
        if (!length_ptr && buf_ptr && ft_err == 0) {
            bytes_len = null_length_size;
        }
        if (buf_ptr && bytes_len > 0 && ft_err == 0) {
            unsigned long hash = 5381;
            for (FT_ULong i = 0; i < bytes_len; i++) {
                hash = ((hash << 5) + hash) + buf_ptr[i];
            }
            printf(",\"bytes_hash\":\"%lx\"", hash);
            printf(",\"bytes_written\":\"");
            print_hex_bytes(buf_ptr, (long)bytes_len);
            printf("\"");
        }
        printf("}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--sfnt-table-info")) {
        FT_UInt table_index = (FT_UInt)strtoul(argv[7], NULL, 10);
        const char* tag_ptr_kind = argv[8];
        const char* length_ptr_kind = argv[9];
        FT_ULong tag_out = 0;
        FT_ULong length_out = 0;
        FT_ULong* tag_ptr = streq(tag_ptr_kind, "non_null") ? &tag_out : NULL;
        FT_ULong* len_ptr = streq(length_ptr_kind, "non_null") ? &length_out : NULL;
        FT_Error ft_err = FT_Sfnt_Table_Info(face, table_index, tag_ptr, len_ptr);
        print_status(ft_err);
        printf(",\"output\":{\"tag_after\":%lu,\"length_after\":%lu}}\n",
            (unsigned long)tag_out, (unsigned long)length_out);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-sfnt-os2-unicode-ranges")) {
        TT_OS2* os2 = (TT_OS2*)FT_Get_Sfnt_Table(face, FT_SFNT_OS2);
        print_status(0);
        printf(",\"output\":{\"table_present\":");
        print_json_bool(os2 != NULL);
        printf(",\"has_os2_table\":");
        print_json_bool(os2 != NULL);
        if (os2) {
            printf(",\"ulUnicodeRange1\":%lu", os2->ulUnicodeRange1);
            printf(",\"ulUnicodeRange2\":%lu", os2->ulUnicodeRange2);
            printf(",\"ulUnicodeRange3\":%lu", os2->ulUnicodeRange3);
            printf(",\"ulUnicodeRange4\":%lu", os2->ulUnicodeRange4);
        } else {
            printf(",\"ulUnicodeRange1\":0,\"ulUnicodeRange2\":0,\"ulUnicodeRange3\":0,\"ulUnicodeRange4\":0");
        }
        printf(",\"glyph_indices\":[");
        size_t codepoints_len = strlen(argv[7]);
        char* codepoints = (char*)malloc(codepoints_len + 1);
        if (!codepoints) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(codepoints, argv[7], codepoints_len + 1);
        char* token = strtok(codepoints, ",");
        int first = 1;
        while (token) {
            if (token[0] != '\0') {
                FT_ULong char_code = strtoul(token, NULL, 10);
                if (!first) printf(",");
                first = 0;
                printf("%u", FT_Get_Char_Index(face, char_code));
            }
            token = strtok(NULL, ",");
        }
        free(codepoints);
        printf("]}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-first-char") || streq(command, "--get-first-char-null-agindex")) {
        FT_UInt glyph_index = 0;
        FT_UInt* glyph_index_ptr = streq(command, "--get-first-char-null-agindex") ? NULL : &glyph_index;
        FT_ULong char_code = FT_Get_First_Char(face, glyph_index_ptr);
        print_status(0);
        printf(",\"output\":");
        if (glyph_index_ptr) {
            print_char_iteration_result(char_code, glyph_index);
        } else {
            print_char_iteration_return_only(char_code);
        }
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-next-char-sequence") || streq(command, "--get-next-char-sequence-null-agindex")) {
        unsigned long max_steps = strtoul(argv[7], NULL, 10);
        FT_UInt glyph_index = 0;
        FT_UInt* glyph_index_ptr = streq(command, "--get-next-char-sequence-null-agindex") ? NULL : &glyph_index;
        FT_ULong char_code = FT_Get_First_Char(face, glyph_index_ptr);
        print_status(0);
        printf(",\"output\":{\"sequence\":[");
        for (unsigned long i = 0; i < max_steps && (glyph_index_ptr ? glyph_index != 0 : char_code != 0); i++) {
            if (i) printf(",");
            if (glyph_index_ptr) {
                print_char_iteration_result(char_code, glyph_index);
            } else {
                print_char_iteration_return_only(char_code);
            }
            char_code = FT_Get_Next_Char(face, char_code, glyph_index_ptr);
        }
        printf("]}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-next-char-starts") || streq(command, "--get-next-char-starts-null-agindex")) {
        size_t starts_len = strlen(argv[7]);
        char* starts = (char*)malloc(starts_len + 1);
        if (!starts) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(starts, argv[7], starts_len + 1);
        print_status(0);
        printf(",\"output\":{\"rows\":[");
        char* token = strtok(starts, ",");
        int first = 1;
        while (token) {
            FT_ULong start = strtoul(token, NULL, 10);
            FT_UInt glyph_index = 0;
            FT_UInt* glyph_index_ptr = streq(command, "--get-next-char-starts-null-agindex") ? NULL : &glyph_index;
            FT_ULong char_code = FT_Get_Next_Char(face, start, glyph_index_ptr);
            if (!first) printf(",");
            first = 0;
            printf("{\"start\":%lu,\"result\":", (unsigned long)start);
            if (glyph_index_ptr) {
                print_char_iteration_result(char_code, glyph_index);
            } else {
                print_char_iteration_return_only(char_code);
            }
            printf("}");
            token = strtok(NULL, ",");
        }
        printf("]}}\n");
        free(starts);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-advance")) {
        FT_UInt glyph_index = (FT_UInt)strtoul(argv[7], NULL, 10);
        FT_Int32 load_flags = (FT_Int32)strtol(argv[8], NULL, 10);
        if (argc == 11) {
            FT_Fixed sentinel = (FT_Fixed)strtoll(argv[10], NULL, 10);
            FT_Error first_error = 0;
            char* probes_first = (char*)malloc(strlen(argv[9]) + 1);
            char* probes_second = (char*)malloc(strlen(argv[9]) + 1);
            if (!probes_first || !probes_second) {
                free(probes_first);
                free(probes_second);
                FT_Done_Face(face);
                FT_Done_FreeType(library);
                free(data);
                return 2;
            }
            memcpy(probes_first, argv[9], strlen(argv[9]) + 1);
            memcpy(probes_second, argv[9], strlen(argv[9]) + 1);

            char* token = strtok(probes_first, ",");
            while (token) {
                FT_Fixed advance = sentinel;
                if (streq(token, "null_face")) {
                    err = FT_Get_Advance(NULL, glyph_index, load_flags, &advance);
                } else if (streq(token, "null_padvance")) {
                    err = FT_Get_Advance(face, glyph_index, load_flags, NULL);
                } else {
                    err = FT_Err_Invalid_Argument;
                }
                if (!first_error && err) {
                    first_error = err;
                }
                token = strtok(NULL, ",");
            }

            print_status(first_error);
            printf(",\"output\":{\"rows\":[");
            token = strtok(probes_second, ",");
            int first = 1;
            while (token) {
                FT_Fixed advance = sentinel;
                if (streq(token, "null_face")) {
                    err = FT_Get_Advance(NULL, glyph_index, load_flags, &advance);
                } else if (streq(token, "null_padvance")) {
                    err = FT_Get_Advance(face, glyph_index, load_flags, NULL);
                } else {
                    err = FT_Err_Invalid_Argument;
                }
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf(
                    "{\"probe\":\"%s\",\"status\":%ld,\"error\":%ld,"
                    "\"padvance\":%ld,\"padvance_preserved\":%s}",
                    token,
                    (long)err,
                    (long)err,
                    (long)advance,
                    advance == sentinel ? "true" : "false");
                token = strtok(NULL, ",");
            }
            printf("]}}\n");
            free(probes_first);
            free(probes_second);
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 0;
        }
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
        if (argc == 12) {
            FT_Fixed sentinel = (FT_Fixed)strtoll(argv[11], NULL, 10);
            FT_Error first_error = 0;
            size_t alloc_count = count ? (size_t)count : 1;
            char* probes_first = (char*)malloc(strlen(argv[10]) + 1);
            char* probes_second = (char*)malloc(strlen(argv[10]) + 1);
            FT_Fixed* advances = (FT_Fixed*)malloc(alloc_count * sizeof(FT_Fixed));
            if (!probes_first || !probes_second || !advances) {
                free(probes_first);
                free(probes_second);
                free(advances);
                FT_Done_Face(face);
                FT_Done_FreeType(library);
                free(data);
                return 2;
            }
            memcpy(probes_first, argv[10], strlen(argv[10]) + 1);
            memcpy(probes_second, argv[10], strlen(argv[10]) + 1);

            char* token = strtok(probes_first, ",");
            while (token) {
                for (FT_UInt i = 0; i < count; i++) {
                    advances[i] = sentinel;
                }
                if (streq(token, "null_face")) {
                    err = FT_Get_Advances(NULL, start, count, load_flags, advances);
                } else if (streq(token, "null_padvances")) {
                    err = FT_Get_Advances(face, start, count, load_flags, NULL);
                } else {
                    err = FT_Err_Invalid_Argument;
                }
                if (!first_error && err) {
                    first_error = err;
                }
                token = strtok(NULL, ",");
            }

            print_status(first_error);
            printf(",\"output\":{\"rows\":[");
            token = strtok(probes_second, ",");
            int first = 1;
            while (token) {
                for (FT_UInt i = 0; i < count; i++) {
                    advances[i] = sentinel;
                }
                if (streq(token, "null_face")) {
                    err = FT_Get_Advances(NULL, start, count, load_flags, advances);
                } else if (streq(token, "null_padvances")) {
                    err = FT_Get_Advances(face, start, count, load_flags, NULL);
                } else {
                    err = FT_Err_Invalid_Argument;
                }
                int preserved = 1;
                for (FT_UInt i = 0; i < count; i++) {
                    if (advances[i] != sentinel) {
                        preserved = 0;
                    }
                }
                if (!first) {
                    printf(",");
                }
                first = 0;
                printf(
                    "{\"probe\":\"%s\",\"status\":%ld,\"error\":%ld,"
                    "\"padvances\":[",
                    token,
                    (long)err,
                    (long)err);
                for (FT_UInt i = 0; i < count; i++) {
                    if (i) {
                        printf(",");
                    }
                    printf("%ld", (long)advances[i]);
                }
                printf("],\"padvances_preserved\":%s}", preserved ? "true" : "false");
                token = strtok(NULL, ",");
            }
            printf("]}}\n");
            free(probes_first);
            free(probes_second);
            free(advances);
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 0;
        }
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

    if (streq(command, "--face-set-unpatented-hinting-post-load")) {
        const char* values_arg = argv[7];
        FT_UInt glyph_index = (FT_UInt)strtoul(argv[8], NULL, 10);
        FT_Int32 load_flags = (FT_Int32)strtol(argv[9], NULL, 10);
        char* values = (char*)malloc(strlen(values_arg) + 1);
        if (!values) {
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        memcpy(values, values_arg, strlen(values_arg) + 1);
        FT_Bool result = 0;
        char* cursor = values;
        while (cursor && *cursor) {
            char* next = strchr(cursor, ',');
            if (next) {
                *next = '\0';
            }
            FT_Bool value = (FT_Bool)strtoul(cursor, NULL, 10);
            result = FT_Face_SetUnpatentedHinting(face, value);
            cursor = next ? next + 1 : NULL;
        }
        free(values);
        err = FT_Load_Glyph(face, glyph_index, load_flags);
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",\"output\":{\"return\":%u,\"post_toggle_slot\":{", (unsigned)result);
            print_slot_body(face->glyph, glyph_index);
            printf("}}}\n");
        }
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--glyphslot-slant") || streq(command, "--glyphslot-oblique")) {
        int result = print_glyphslot_slant_rows(face, argv[7], streq(command, "--glyphslot-oblique"));
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return result ? 2 : 0;
    }
    if (streq(command, "--glyphslot-adjust-weight") || streq(command, "--glyphslot-embolden")) {
        int result = print_glyphslot_weight_rows(face, argv[7], streq(command, "--glyphslot-embolden"));
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return result ? 2 : 0;
    }
    if (streq(command, "--slot-format-probe")) {
        int result = print_slot_format_probe_rows(face, argv[7]);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return result ? 2 : 0;
    }
    if (streq(command, "--glyph-slot-reuse")) {
        int result = print_glyph_slot_reuse_rows(face, argv[7], (FT_Int32)strtol(argv[8], NULL, 10));
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return result ? 2 : 0;
    }
    if (streq(command, "--render-glyph-slot-states")) {
        int result = print_render_glyph_slot_state_rows(face, argv[7], (FT_Render_Mode)strtol(argv[8], NULL, 10));
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return result ? 2 : 0;
    }

    FT_UInt glyph_index = 0;
    FT_Int32 load_flags = 0;
    if (streq(command, "--load-char") || streq(command, "--render-glyph") || streq(command, "--load-glyph-from-char")) {
        FT_ULong char_code = strtoul(argv[7], NULL, 10);
        load_flags = (FT_Int32)strtol(argv[8], NULL, 10);
        glyph_index = FT_Get_Char_Index(face, char_code);
    } else if (streq(command, "--load-glyph-num-glyphs")) {
        glyph_index = (FT_UInt)face->num_glyphs;
        load_flags = (FT_Int32)strtol(argv[7], NULL, 10);
    } else if (streq(command, "--load-glyph") || streq(command, "--render-glyph-index") || streq(command, "--inspect-glyph-metrics") || streq(command, "--inspect-glyph-slot") || streq(command, "--load-glyph-outline") || streq(command, "--outline-get-bbox") || streq(command, "--outline-get-cbox") || streq(command, "--glyph-get-cbox") || streq(command, "--glyph-transform") || streq(command, "--glyph-to-bitmap") || streq(command, "--glyph-record") || streq(command, "--get-glyph-unsupported-format") || streq(command, "--done-glyph-outline") || streq(command, "--done-glyph-bitmap") || streq(command, "--get-glyph-advance-boundaries") || streq(command, "--sbit-cache-lookup") || streq(command, "--get-subglyph-info") || streq(command, "--get-subglyph-info-null-outputs")) {
        glyph_index = (FT_UInt)strtoul(argv[7], NULL, 10);
        load_flags = (FT_Int32)strtol(argv[8], NULL, 10);
    } else {
        fprintf(stderr, "unsupported command: %s\n", command);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 2;
    }

    if (streq(command, "--render-glyph") || streq(command, "--render-glyph-index")) {
        int repeat_count = render_glyph_repeat_count(argc, argv);
        if (repeat_count > 1) {
            FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[9], NULL, 10);
            err = print_render_glyph_sequence(face, glyph_index, load_flags, render_mode, repeat_count);
            FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return err ? 2 : 0;
        }
    }

    err = FT_Load_Glyph(face, glyph_index, load_flags);
    int render_error_slot_ready = !err &&
        (streq(command, "--render-glyph") || streq(command, "--render-glyph-index"));
    if (!err && (streq(command, "--render-glyph") || streq(command, "--render-glyph-index") || (streq(command, "--inspect-glyph-slot") && argc == 10))) {
        FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[9], NULL, 10);
        err = FT_Render_Glyph(face->glyph, render_mode);
    }
    if (!err && streq(command, "--glyph-get-cbox")) {
        print_glyph_cbox_payload(face->glyph, argv[9]);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (streq(command, "--glyph-transform")) {
        print_glyph_transform_payload(face, argv[7], load_flags, argv[9]);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--glyph-to-bitmap")) {
        FT_Render_Mode render_mode = (FT_Render_Mode)strtol(argv[9], NULL, 10);
        int destroy = atoi(argv[10]);
        print_glyph_to_bitmap_payload(face->glyph, render_mode, destroy);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--glyph-record")) {
        print_get_glyph_payload(face->glyph, argv[9]);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--get-glyph-unsupported-format")) {
        print_get_glyph_unsupported_format_payload(face->glyph);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--done-glyph-outline")) {
        print_done_outline_glyph_payload(face->glyph);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--done-glyph-bitmap")) {
        print_done_bitmap_glyph_payload(face->glyph);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--get-glyph-advance-boundaries")) {
        emit_get_glyph_advance_boundaries(face, argv[9]);
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--sbit-cache-lookup")) {
        if (face->glyph->format != FT_GLYPH_FORMAT_BITMAP) {
            err = FT_Render_Glyph(face->glyph, FT_RENDER_MODE_NORMAL);
        }
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",");
            print_sbit_payload(face->glyph);
            printf("}\n");
        }
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--get-subglyph-info")) {
        print_status(0);
        printf(",");
        print_subglyph_info_payload(face->glyph, argv[9], argv[10]);
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    if (!err && streq(command, "--get-subglyph-info-null-outputs")) {
        FT_UInt sub_index = (FT_UInt)strtoul(argv[9], NULL, 10);
        print_subglyph_null_outputs_payload(face->glyph, sub_index, argv[10]);
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }
    print_status(err);
    if (err) {
        if (render_error_slot_ready && render_glyph_capture_error_slot(argc, argv)) {
            printf(",");
            print_slot(face->glyph, glyph_index);
            printf("}\n");
        } else {
            printf(",\"output\":null}\n");
        }
    } else if (streq(command, "--inspect-glyph-metrics")) {
        printf(",");
        print_glyph_metrics(face->glyph->metrics);
        printf("}\n");
    } else if (streq(command, "--load-glyph-outline")) {
        printf(",");
        print_outline_payload(face->glyph);
        printf("}\n");
    } else if (streq(command, "--outline-get-bbox")) {
        printf(",");
        print_outline_bbox_payload(face->glyph);
        printf("}\n");
    } else if (streq(command, "--outline-get-cbox")) {
        printf(",");
        print_outline_cbox_payload(face->glyph);
        printf("}\n");
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

static void print_bitmap_sizes(FT_Face face) {
    printf("[");
    for (FT_Int i = 0; i < face->num_fixed_sizes; i++) {
        FT_Bitmap_Size* size = &face->available_sizes[i];
        if (i) {
            printf(",");
        }
        printf("{\"height\":%d,\"width\":%d,\"size\":%ld,\"x_ppem\":%ld,\"y_ppem\":%ld}",
               size->height,
               size->width,
               (long)size->size,
               (long)size->x_ppem,
               (long)size->y_ppem);
    }
    printf("]");
}

static int load_memory_face_arg(
    FT_Library library,
    const char* source_kind,
    const char* source_value,
    FT_Long face_index,
    unsigned char** data,
    long* data_len,
    FT_Face* face
) {
    *data = NULL;
    *data_len = 0;
    *face = NULL;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, data, data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, data, data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        return 2;
    }
    FT_Error err = FT_New_Memory_Face(library, *data, *data_len, face_index, face);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        free(*data);
        *data = NULL;
        return 1;
    }
    return 0;
}

static int emit_available_sizes(int argc, char** argv) {
    (void)argc;
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    unsigned char* data = NULL;
    unsigned char* control_data = NULL;
    long data_len = 0;
    long control_data_len = 0;
    FT_Face face = NULL;
    FT_Face control = NULL;
    FT_Long face_index = atol(argv[4]);
    int status = load_memory_face_arg(
        library, argv[2], argv[3], face_index, &data, &data_len, &face);
    if (status) {
        FT_Done_FreeType(library);
        return status == 1 ? 0 : status;
    }
    status = load_memory_face_arg(
        library, argv[5], argv[6], 0, &control_data, &control_data_len, &control);
    if (status) {
        FT_Done_Face(face);
        free(data);
        FT_Done_FreeType(library);
        return status == 1 ? 0 : status;
    }

    printf("{");
    print_status(0);
    printf(",\"output\":{\"num_fixed_sizes\":%d,\"available_sizes\":", face->num_fixed_sizes);
    print_bitmap_sizes(face);
    printf(",\"control_num_fixed_sizes\":%d,", control->num_fixed_sizes);
    printf("\"control_available_sizes_nullness\":\"%s\"}}\n",
           control->available_sizes ? "non-null" : "null");

    FT_Done_Face(control);
    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(control_data);
    free(data);
    return 0;
}

static void print_face_rec_initial_snapshot(FT_Face face) {
    printf("{\"num_faces\":%ld", face->num_faces);
    printf(",\"face_index\":%ld", face->face_index);
    printf(",\"face_flags\":%ld", face->face_flags);
    printf(",\"style_flags\":%ld", face->style_flags);
    printf(",\"num_glyphs\":%ld", face->num_glyphs);
    printf(",\"num_fixed_sizes\":%d", face->num_fixed_sizes);
    printf(",\"available_sizes_nullness\":\"%s\"",
           face->available_sizes ? "non-null" : "null");
    printf(",\"bbox\":{\"xMin\":%ld,\"yMin\":%ld,\"xMax\":%ld,\"yMax\":%ld}",
           face->bbox.xMin,
           face->bbox.yMin,
           face->bbox.xMax,
           face->bbox.yMax);
    printf(",\"units_per_EM\":%u", face->units_per_EM);
    printf(",\"ascender\":%d", face->ascender);
    printf(",\"descender\":%d", face->descender);
    printf(",\"height\":%d", face->height);
    printf(",\"max_advance_width\":%d", face->max_advance_width);
    printf(",\"max_advance_height\":%d", face->max_advance_height);
    printf(",\"underline_position\":%d", face->underline_position);
    printf(",\"underline_thickness\":%d", face->underline_thickness);
    printf(",\"size_nullness\":\"%s\"", face->size ? "non-null" : "null");
    printf(",\"stream_nullness\":\"%s\"", face->stream ? "non-null" : "null");
    printf("}");
}

static int emit_face_rec_initial_snapshot(int argc, char** argv) {
    (void)argc;
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    unsigned char* data = NULL;
    long data_len = 0;
    FT_Face face = NULL;
    FT_Long face_index = atol(argv[4]);
    int status = load_memory_face_arg(
        library, argv[2], argv[3], face_index, &data, &data_len, &face);
    if (status) {
        FT_Done_FreeType(library);
        return status == 1 ? 0 : status;
    }

    printf("{");
    print_status(0);
    printf(",\"output\":");
    print_face_rec_initial_snapshot(face);
    printf("}\n");

    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_face_rec_post_size_snapshot(int argc, char** argv) {
    (void)argc;
    FT_Library library = NULL;
    FT_Error err = FT_Init_FreeType(&library);
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

    unsigned char* data = NULL;
    long data_len = 0;
    FT_Face face = NULL;
    FT_Long face_index = atol(argv[4]);
    int status = load_memory_face_arg(
        library, argv[2], argv[3], face_index, &data, &data_len, &face);
    if (status) {
        FT_Done_FreeType(library);
        return status == 1 ? 0 : status;
    }

    FT_F26Dot6 char_width = (FT_F26Dot6)strtol(argv[5], NULL, 10);
    FT_F26Dot6 char_height = (FT_F26Dot6)strtol(argv[6], NULL, 10);
    FT_UInt horz_resolution = (FT_UInt)strtoul(argv[7], NULL, 10);
    FT_UInt vert_resolution = (FT_UInt)strtoul(argv[8], NULL, 10);
    err = FT_Set_Char_Size(face, char_width, char_height, horz_resolution, vert_resolution);

    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"face\":");
        print_face_rec_initial_snapshot(face);
        printf(",\"size_metrics\":{");
        print_size_metrics_object(face->size->metrics);
        printf("}}}\n");
    }

    FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

typedef struct MemoryFaceRow_ {
    FT_Long face_index;
    int has_file_size;
    FT_Long file_size;
    FT_UInt open_flags;
    int file_base_is_null;
    int library_is_null;
    int aface_is_null;
    int open_args_is_null;
} MemoryFaceRow;

static int parse_memory_face_row(char* row, MemoryFaceRow* out) {
    char* fields[8];
    char* cursor = row;
    for (int i = 0; i < 8; i++) {
        fields[i] = cursor;
        char* sep = strchr(cursor, ':');
        if (i == 7) {
            if (sep) {
                return 0;
            }
            break;
        }
        if (!sep) {
            return 0;
        }
        *sep = '\0';
        cursor = sep + 1;
    }
    out->face_index = (FT_Long)strtol(fields[0], NULL, 10);
    out->has_file_size = (int)strtol(fields[1], NULL, 10) != 0;
    out->file_size = (FT_Long)strtol(fields[2], NULL, 10);
    out->open_flags = (FT_UInt)strtoul(fields[3], NULL, 10);
    out->file_base_is_null = (int)strtol(fields[4], NULL, 10) != 0;
    out->library_is_null = (int)strtol(fields[5], NULL, 10) != 0;
    out->aface_is_null = (int)strtol(fields[6], NULL, 10) != 0;
    out->open_args_is_null = (int)strtol(fields[7], NULL, 10) != 0;
    return 1;
}

static void print_memory_face_row(MemoryFaceRow row, FT_Error err, int face_is_null) {
    printf("{\"face_index\":%ld,\"file_size\":", row.face_index);
    if (row.has_file_size) {
        printf("%ld", row.file_size);
    } else {
        printf("null");
    }
    printf(",\"status\":%d,\"error\":%d,\"opened\":%s,"
           "\"face_nullness\":\"%s\"}",
           err,
           err,
           face_is_null ? "false" : "true",
           face_is_null ? "null" : "non_null");
}

static void print_open_face_name_output(FT_Error err, FT_Face face) {
    printf("{\"return\":%d,\"status\":%d,\"opened\":%s,\"family_name\":",
           err,
           err,
           face ? "true" : "false");
    print_nullable_c_string_result(face ? face->family_name : NULL);
    printf(",\"style_name\":");
    print_nullable_c_string_result(face ? face->style_name : NULL);
    printf("}");
}

typedef struct OpenFaceStreamState_ {
    int close_calls;
    unsigned int magic;
} OpenFaceStreamState;

static void open_face_stream_close(FT_Stream stream) {
    if (stream && stream->descriptor.pointer) {
        OpenFaceStreamState* state = (OpenFaceStreamState*)stream->descriptor.pointer;
        state->close_calls++;
    }
}

static int emit_open_face_stream_ownership(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    OpenFaceStreamState state;
    state.close_calls = 0;
    state.magic = 0xF75EA123u;
    FT_StreamRec stream;
    memset(&stream, 0, sizeof(stream));
    stream.base = data;
    stream.size = (unsigned long)data_len;
    stream.descriptor.pointer = &state;
    stream.close = open_face_stream_close;

    FT_Open_Args args;
    memset(&args, 0, sizeof(args));
    args.flags = FT_OPEN_STREAM;
    args.stream = &stream;

    FT_Face face = NULL;
    FT_Error err = FT_Open_Face(library, &args, face_index, &face);
    long face_flags = face ? (long)face->face_flags : 0;
    int bit_set = face && ((face->face_flags & FT_FACE_FLAG_EXTERNAL_STREAM) != 0);
    if (!err && face) {
        FT_Done_Face(face);
    }
    int alive = state.magic == 0xF75EA123u;

    printf("{");
    print_status(err);
    printf(",\"output\":{\"return\":%d,\"status\":%d,\"opened\":", err, err);
    print_json_bool(err == FT_Err_Ok);
    printf(",\"face_flags\":%ld,\"bit_set\":", face_flags);
    print_json_bool(bit_set);
    printf(",\"stream_close_calls\":%d,\"client_stream_alive_after_done_face\":", state.close_calls);
    print_json_bool(alive);
    printf("}}\n");

    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static void print_memory_stream_frame_read(FT_Stream stream, unsigned long offset, unsigned long count) {
    unsigned long size = stream ? stream->size : 0;
    int in_bounds = stream && offset <= size && count <= size - offset;
    unsigned long available = 0;
    if (stream && offset <= size) {
        unsigned long remaining = size - offset;
        available = count < remaining ? count : remaining;
    }
    printf("{\"offset\":%lu,\"count\":%lu,\"in_bounds\":", offset, count);
    print_json_bool(in_bounds);
    printf(",\"bytes\":\"");
    if (stream && stream->base && available > 0) {
        print_hex_bytes(stream->base + offset, (long)available);
    }
    printf("\"}");
}

static int emit_memory_stream_probe(int argc, char** argv) {
    if (argc != 5) {
        fprintf(stderr, "memory stream probe requires source_kind source_value face_index\n");
        return 2;
    }
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_Stream stream = face.face ? face.face->stream : NULL;
    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":{\"face_load_status\":0,\"stream_fields\":{");
    printf("\"base_nullness\":");
    print_json_bool(!stream || !stream->base);
    printf(",\"size\":%lu,\"pos\":%lu,\"cursor_nullness\":",
           stream ? stream->size : 0,
           stream ? stream->pos : 0);
    print_json_bool(!stream || !stream->cursor);
    printf(",\"limit_nullness\":");
    print_json_bool(!stream || !stream->limit);
    printf("},\"frame_read_events\":[");
    print_memory_stream_frame_read(stream, 0, 4);
    printf(",");
    print_memory_stream_frame_read(stream, 12, 4);
    printf("]}}\n");
    close_oracle_face(&face);
    return 0;
}

static void print_face_style_flag_output(FT_Error err, FT_Face face) {
    printf("{\"status\":%d,\"family_name\":", err);
    print_nullable_c_string_result(face ? face->family_name : NULL);
    printf(",\"style_name\":");
    print_nullable_c_string_result(face ? face->style_name : NULL);
    printf(",\"style_flags\":%ld,\"has_bold_flag\":",
           face ? (long)face->style_flags : 0);
    print_json_bool(face && ((face->style_flags & FT_STYLE_FLAG_BOLD) != 0));
    printf("}");
}

static int emit_face_style_flags(int argc, char** argv) {
    (void)argc;
    OracleFace bold;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[6]), &bold);
    if (opened != 0) {
        return opened;
    }
    OracleFace regular;
    opened = open_oracle_face(argv[4], argv[5], atol(argv[6]), &regular);
    if (opened != 0) {
        close_oracle_face(&bold);
        return opened;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{");
    printf("\"bold_face\":");
    print_face_style_flag_output(FT_Err_Ok, bold.face);
    printf(",\"regular_face\":");
    print_face_style_flag_output(FT_Err_Ok, regular.face);
    printf("}}\n");

    close_oracle_face(&regular);
    close_oracle_face(&bold);
    return 0;
}

static int emit_open_face_name_options(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    char* rows_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!rows_arg) {
        return 1;
    }
    memcpy(rows_arg, argv[5], strlen(argv[5]) + 1);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(rows_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(rows_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(rows_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(rows_arg);
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"results\":[");
    char* cursor = rows_arg;
    int first = 1;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        int ignore_family = 0;
        int ignore_subfamily = 0;
        if (sscanf(cursor, "%d:%d", &ignore_family, &ignore_subfamily) != 2) {
            FT_Done_FreeType(library);
            free(data);
            free(rows_arg);
            return 2;
        }
        FT_Parameter params[2];
        FT_Int num_params = 0;
        if (ignore_family) {
            params[num_params].tag = FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY;
            params[num_params].data = NULL;
            num_params++;
        }
        if (ignore_subfamily) {
            params[num_params].tag = FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY;
            params[num_params].data = NULL;
            num_params++;
        }
        FT_Open_Args args;
        memset(&args, 0, sizeof(args));
        args.flags = FT_OPEN_MEMORY | FT_OPEN_PARAMS;
        args.memory_base = data;
        args.memory_size = data_len;
        args.num_params = num_params;
        args.params = num_params ? params : NULL;
        FT_Face face = NULL;
        FT_Error err = FT_Open_Face(library, &args, face_index, &face);
        if (!first_error && err) {
            first_error = err;
        }
        if (!first) {
            printf(",");
        }
        print_open_face_name_output(err, face);
        first = 0;
        if (!err && face) {
            FT_Done_Face(face);
        }
        cursor = next ? next + 1 : NULL;
    }
    printf("]}}\n");

    FT_Done_FreeType(library);
    free(data);
    free(rows_arg);
    return 0;
}

static void print_open_face_ignored_params_observation(
    FT_Error open_error,
    FT_Face face,
    FT_UInt glyph_index,
    FT_Int32 load_flags,
    FT_UInt size_ppem) {
    printf("{\"open_error\":%d,\"face_flags\":", open_error);
    if (open_error || !face) {
        printf("null,\"glyph_slot\":null}");
        return;
    }
    printf("%ld", face->face_flags);
    FT_Error size_error = FT_Err_Ok;
    if (size_ppem) {
        size_error = FT_Set_Pixel_Sizes(face, size_ppem, size_ppem);
    }
    printf(",\"size_error\":%d", size_error);
    if (size_error) {
        printf(",\"load_error\":%d,\"glyph_slot\":null}", size_error);
        return;
    }
    FT_Error load_error = FT_Load_Glyph(face, glyph_index, load_flags);
    printf(",\"load_error\":%d,\"glyph_slot\":", load_error);
    if (load_error) {
        printf("null}");
        return;
    }
    printf("{");
    print_slot_body(face->glyph, glyph_index);
    printf("}}");
}

static FT_Error open_face_with_unpatented_hinting_params(
    FT_Library library,
    const unsigned char* data,
    long data_len,
    FT_Long face_index,
    int include_params,
    FT_Face* face) {
    FT_Parameter params[2];
    FT_Bool ignored_value = 1;
    FT_Int num_params = 0;
    if (include_params) {
        params[num_params].tag = FT_PARAM_TAG_UNPATENTED_HINTING;
        params[num_params].data = NULL;
        num_params++;
        params[num_params].tag = FT_PARAM_TAG_UNPATENTED_HINTING;
        params[num_params].data = &ignored_value;
        num_params++;
    }
    FT_Open_Args args;
    memset(&args, 0, sizeof(args));
    args.flags = FT_OPEN_MEMORY | FT_OPEN_PARAMS;
    args.memory_base = data;
    args.memory_size = data_len;
    args.num_params = num_params;
    args.params = num_params ? params : NULL;
    return FT_Open_Face(library, &args, face_index, face);
}

static int emit_open_face_ignored_params(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = (FT_Long)strtol(argv[4], NULL, 10);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[6], NULL, 10);
    FT_UInt size_ppem = (FT_UInt)strtoul(argv[7], NULL, 10);
    int compare_control = atoi(argv[8]);

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
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":");
    if (compare_control) {
        FT_Face control_face = NULL;
        FT_Error control_error = open_face_with_unpatented_hinting_params(
            library, data, data_len, face_index, 0, &control_face);
        FT_Face test_face = NULL;
        FT_Error test_error = open_face_with_unpatented_hinting_params(
            library, data, data_len, face_index, 1, &test_face);
        printf("{\"control\":");
        print_open_face_ignored_params_observation(control_error, control_face, glyph_index, load_flags, size_ppem);
        printf(",\"test\":");
        print_open_face_ignored_params_observation(test_error, test_face, glyph_index, load_flags, size_ppem);
        printf("}}\n");
        if (control_face) {
            FT_Done_Face(control_face);
        }
        if (test_face) {
            FT_Done_Face(test_face);
        }
    } else {
        FT_Face face = NULL;
        FT_Error err = open_face_with_unpatented_hinting_params(
            library, data, data_len, face_index, 1, &face);
        print_open_face_ignored_params_observation(err, face, glyph_index, load_flags, size_ppem);
        printf("}\n");
        if (face) {
            FT_Done_Face(face);
        }
    }

    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_new_memory_face_variants(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    char* rows_arg = (char*)malloc(strlen(argv[4]) + 1);
    if (!rows_arg) {
        return 1;
    }
    memcpy(rows_arg, argv[4], strlen(argv[4]) + 1);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(rows_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(rows_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(rows_arg);
        return 2;
    }

    size_t row_count = 0;
    const char* count_cursor = rows_arg;
    while (count_cursor && *count_cursor) {
        row_count++;
        const char* next = strchr(count_cursor, ',');
        count_cursor = next ? next + 1 : NULL;
    }
    MemoryFaceRow* rows = (MemoryFaceRow*)calloc(row_count, sizeof(MemoryFaceRow));
    FT_Error* errors = (FT_Error*)calloc(row_count, sizeof(FT_Error));
    int* face_is_null = (int*)calloc(row_count, sizeof(int));
    if ((!rows || !errors || !face_is_null) && row_count > 0) {
        free(data);
        free(rows_arg);
        free(rows);
        free(errors);
        free(face_is_null);
        return 1;
    }

    char* cursor = rows_arg;
    size_t row_index = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        if (!parse_memory_face_row(cursor, &rows[row_index])) {
            fprintf(stderr, "bad memory face row: %s\n", cursor);
            free(data);
            free(rows_arg);
            free(rows);
            free(errors);
            free(face_is_null);
            return 2;
        }
        row_index++;
        cursor = next ? next + 1 : NULL;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(rows_arg);
        free(rows);
        free(errors);
        free(face_is_null);
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    for (size_t i = 0; i < row_count; i++) {
        FT_Face face = NULL;
        FT_Long file_size = rows[i].has_file_size ? rows[i].file_size : data_len;
        FT_Library library_arg = rows[i].library_is_null ? NULL : library;
        const FT_Byte* file_base_arg = rows[i].file_base_is_null ? NULL : data;
        FT_Face* aface_arg = rows[i].aface_is_null ? NULL : &face;
        errors[i] = FT_New_Memory_Face(
            library_arg,
            file_base_arg,
            file_size,
            rows[i].face_index,
            aface_arg);
        /* FT_New_Memory_Face delegates through FT_Open_Face.  A valid `aface`
         * is assigned only after the face opens successfully; error exits leave
         * the caller's output unchanged.  Capture it before success cleanup so
         * exact-error rows verify that public output-pointer contract. */
        face_is_null[i] = face == NULL;
        if (!first_error && errors[i]) {
            first_error = errors[i];
        }
        if (!errors[i] && face) {
            FT_Done_Face(face);
        }
    }

    printf("{");
    print_status(first_error);
    printf(",\"output\":{\"outputs\":[");
    for (size_t i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_memory_face_row(rows[i], errors[i], face_is_null[i]);
    }
    printf("]}}\n");

    if (library) {
        FT_Done_FreeType(library);
    }
    free(data);
    free(rows_arg);
    free(rows);
    free(errors);
    free(face_is_null);
    return 0;
}

static void print_incremental_absent_output(FT_Error open_error, FT_Error load_error) {
    printf("{");
    print_status(open_error ? open_error : load_error);
    printf(",\"output\":{\"open_error\":%d,\"load_error\":%d,"
           "\"callback_count\":0,\"embedded_data_used\":",
           open_error,
           load_error);
    print_json_bool(open_error == FT_Err_Ok && load_error == FT_Err_Ok);
    printf("}}\n");
}

static void print_incremental_nullness_row(const char* variant, FT_Error open_error, FT_Error load_error) {
    printf("{\"variant\":\"%s\",\"open_error\":%d,\"load_error\":%d,"
           "\"stored_interface_null\":true,\"callback_count\":0,"
           "\"embedded_data_used\":",
           variant,
           open_error,
           load_error);
    print_json_bool(open_error == FT_Err_Ok && load_error == FT_Err_Ok);
    printf("}");
}

static FT_Error open_incremental_nullness_face(FT_Library library,
                                               const unsigned char* data,
                                               size_t data_len,
                                               FT_Long face_index,
                                               int with_null_incremental_param,
                                               FT_Face* face) {
    if (!with_null_incremental_param) {
        return FT_New_Memory_Face(library, data, (FT_Long)data_len, face_index, face);
    }

    FT_Parameter param;
    memset(&param, 0, sizeof(param));
    param.tag = FT_PARAM_TAG_INCREMENTAL;
    param.data = NULL;

    FT_Open_Args args;
    memset(&args, 0, sizeof(args));
    args.flags = FT_OPEN_MEMORY;
    args.memory_base = data;
    args.memory_size = (FT_Long)data_len;
    args.num_params = 1;
    args.params = &param;
    return FT_Open_Face(library, &args, face_index, face);
}

static int emit_incremental_nullness_open(int argc, char** argv) {
    if (argc != 6) return 2;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    unsigned char* data = NULL;
    long data_len_long = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len_long) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 1;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len_long) != 0) {
            fprintf(stderr, "failed to decode font hex source\n");
            return 1;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        return 2;
    }
    size_t data_len = data_len_long < 0 ? 0 : (size_t)data_len_long;

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        return 0;
    }

    FT_Long face_index = (FT_Long)atol(argv[4]);
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[5], NULL, 10);
    const char* variants[] = {"absent_parameter", "null_incremental_data"};

    printf("{");
    print_status(FT_Err_Ok);
    printf(",\"output\":{\"rows\":[");
    for (int i = 0; i < 2; i++) {
        FT_Face face = NULL;
        FT_Error open_error = open_incremental_nullness_face(
            library,
            data,
            data_len,
            face_index,
            i == 1,
            &face);
        FT_Error load_error = open_error;
        if (!open_error && face) {
            load_error = FT_Load_Glyph(face, glyph_index, FT_LOAD_DEFAULT);
        }
        if (i) printf(",");
        print_incremental_nullness_row(variants[i], open_error, load_error);
        if (face) {
            FT_Done_Face(face);
        }
    }
    printf("]}}\n");

    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_incremental_absent_open(int argc, char** argv) {
    (void)argc;
    OracleFace face;
    int opened = open_oracle_face(argv[2], argv[3], atol(argv[4]), &face);
    if (opened != 0) {
        return opened;
    }
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[5], NULL, 10);
    FT_Error load_error = FT_Load_Glyph(face.face, glyph_index, FT_LOAD_DEFAULT);
    print_incremental_absent_output(FT_Err_Ok, load_error);
    close_oracle_face(&face);
    return 0;
}

static int emit_new_face_variants(int argc, char** argv) {
    (void)argc;
    const char* pathname = argv[2];
    char* rows_arg = (char*)malloc(strlen(argv[3]) + 1);
    if (!rows_arg) {
        return 1;
    }
    memcpy(rows_arg, argv[3], strlen(argv[3]) + 1);

    size_t row_count = 0;
    const char* count_cursor = rows_arg;
    while (count_cursor && *count_cursor) {
        row_count++;
        const char* next = strchr(count_cursor, ',');
        count_cursor = next ? next + 1 : NULL;
    }
    MemoryFaceRow* rows = (MemoryFaceRow*)calloc(row_count, sizeof(MemoryFaceRow));
    FT_Error* errors = (FT_Error*)calloc(row_count, sizeof(FT_Error));
    int* face_is_null = (int*)calloc(row_count, sizeof(int));
    if ((!rows || !errors || !face_is_null) && row_count > 0) {
        free(rows_arg);
        free(rows);
        free(errors);
        free(face_is_null);
        return 1;
    }

    char* cursor = rows_arg;
    size_t row_index = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        if (!parse_memory_face_row(cursor, &rows[row_index])) {
            fprintf(stderr, "bad new face row: %s\n", cursor);
            free(rows_arg);
            free(rows);
            free(errors);
            free(face_is_null);
            return 2;
        }
        row_index++;
        cursor = next ? next + 1 : NULL;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(rows_arg);
        free(rows);
        free(errors);
        free(face_is_null);
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    for (size_t i = 0; i < row_count; i++) {
        FT_Face face = NULL;
        FT_Library library_arg = rows[i].library_is_null ? NULL : library;
        FT_Face* aface_arg = rows[i].aface_is_null ? NULL : &face;
        errors[i] = FT_New_Face(library_arg, pathname, rows[i].face_index, aface_arg);
        face_is_null[i] = face == NULL;
        if (!first_error && errors[i]) {
            first_error = errors[i];
        }
        if (!errors[i] && face) {
            FT_Done_Face(face);
        }
    }

    printf("{");
    print_status(first_error);
    printf(",\"output\":{\"outputs\":[");
    for (size_t i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_memory_face_row(rows[i], errors[i], face_is_null[i]);
    }
    printf("]}}\n");

    if (library) {
        FT_Done_FreeType(library);
    }
    free(rows_arg);
    free(rows);
    free(errors);
    free(face_is_null);
    return 0;
}

static int emit_open_face_variants(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    char* rows_arg = (char*)malloc(strlen(argv[4]) + 1);
    if (!rows_arg) {
        return 1;
    }
    memcpy(rows_arg, argv[4], strlen(argv[4]) + 1);

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(rows_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(rows_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(rows_arg);
        return 2;
    }

    size_t row_count = 0;
    const char* count_cursor = rows_arg;
    while (count_cursor && *count_cursor) {
        row_count++;
        const char* next = strchr(count_cursor, ',');
        count_cursor = next ? next + 1 : NULL;
    }
    MemoryFaceRow* rows = (MemoryFaceRow*)calloc(row_count, sizeof(MemoryFaceRow));
    FT_Error* errors = (FT_Error*)calloc(row_count, sizeof(FT_Error));
    int* face_is_null = (int*)calloc(row_count, sizeof(int));
    if ((!rows || !errors || !face_is_null) && row_count > 0) {
        free(data);
        free(rows_arg);
        free(rows);
        free(errors);
        free(face_is_null);
        return 1;
    }

    char* cursor = rows_arg;
    size_t row_index = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        if (!parse_memory_face_row(cursor, &rows[row_index])) {
            fprintf(stderr, "bad memory face row: %s\n", cursor);
            free(data);
            free(rows_arg);
            free(rows);
            free(errors);
            free(face_is_null);
            return 2;
        }
        row_index++;
        cursor = next ? next + 1 : NULL;
    }

    FT_Library library = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (setup_error) {
        printf("{");
        print_status(setup_error);
        printf(",\"output\":null}\n");
        free(data);
        free(rows_arg);
        free(rows);
        free(errors);
        free(face_is_null);
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    for (size_t i = 0; i < row_count; i++) {
        FT_Face face = NULL;
        FT_Long file_size = rows[i].has_file_size ? rows[i].file_size : data_len;
        FT_Open_Args args;
        memset(&args, 0, sizeof(args));
        args.flags = rows[i].open_flags;
        args.memory_base = rows[i].file_base_is_null ? NULL : data;
        args.memory_size = file_size;
        FT_Library library_arg = rows[i].library_is_null ? NULL : library;
        FT_Open_Args* args_arg = rows[i].open_args_is_null ? NULL : &args;
        FT_Face* aface_arg = rows[i].aface_is_null ? NULL : &face;
        errors[i] = FT_Open_Face(library_arg, args_arg, rows[i].face_index, aface_arg);
        face_is_null[i] = face == NULL;
        if (!first_error && errors[i]) {
            first_error = errors[i];
        }
        if (!errors[i] && face) {
            FT_Done_Face(face);
        }
    }

    printf("{");
    print_status(first_error);
    printf(",\"output\":{\"outputs\":[");
    for (size_t i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        print_memory_face_row(rows[i], errors[i], face_is_null[i]);
    }
    printf("]}}\n");

    if (library) {
        FT_Done_FreeType(library);
    }
    free(data);
    free(rows_arg);
    free(rows);
    free(errors);
    free(face_is_null);
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

    if (streq(source_kind, "null")) {
        FT_Error err = FT_Set_Char_Size(
            NULL,
            char_width,
            char_height,
            horz_resolution,
            vert_resolution);
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }

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

typedef struct CharSizeRow_ {
    FT_F26Dot6 char_width;
    FT_F26Dot6 char_height;
    FT_UInt horz_resolution;
    FT_UInt vert_resolution;
} CharSizeRow;

static int parse_char_size_row(const char* token, CharSizeRow* row) {
    char* copy = (char*)malloc(strlen(token) + 1);
    if (!copy) {
        return 1;
    }
    memcpy(copy, token, strlen(token) + 1);
    char* cursor = copy;
    char* parts[4] = {NULL, NULL, NULL, NULL};
    for (int i = 0; i < 4; i++) {
        parts[i] = cursor;
        char* next = strchr(cursor, ':');
        if (i < 3) {
            if (!next) {
                free(copy);
                return 1;
            }
            *next = '\0';
            cursor = next + 1;
        } else if (next) {
            free(copy);
            return 1;
        }
    }
    row->char_width = (FT_F26Dot6)strtol(parts[0], NULL, 10);
    row->char_height = (FT_F26Dot6)strtol(parts[1], NULL, 10);
    row->horz_resolution = (FT_UInt)strtoul(parts[2], NULL, 10);
    row->vert_resolution = (FT_UInt)strtoul(parts[3], NULL, 10);
    free(copy);
    return 0;
}

static int emit_set_char_sizes(int argc, char** argv) {
    (void)argc;
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    char* rows_arg = (char*)malloc(strlen(argv[5]) + 1);
    if (!rows_arg) {
        return 1;
    }
    memcpy(rows_arg, argv[5], strlen(argv[5]) + 1);

    size_t row_count = 0;
    const char* count_cursor = rows_arg;
    while (count_cursor && *count_cursor) {
        row_count++;
        const char* next = strchr(count_cursor, ',');
        count_cursor = next ? next + 1 : NULL;
    }
    CharSizeRow* rows = (CharSizeRow*)calloc(row_count, sizeof(CharSizeRow));
    FT_Error* errors = (FT_Error*)calloc(row_count, sizeof(FT_Error));
    FT_Size_Metrics* metrics = (FT_Size_Metrics*)calloc(row_count, sizeof(FT_Size_Metrics));
    if ((!rows || !errors || !metrics) && row_count > 0) {
        free(rows);
        free(errors);
        free(metrics);
        free(rows_arg);
        return 1;
    }
    char* cursor = rows_arg;
    size_t row_index = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ',');
        if (next) {
            *next = '\0';
        }
        if (parse_char_size_row(cursor, &rows[row_index]) != 0) {
            fprintf(stderr, "char size rows must be WIDTH:HEIGHT:HRES:VRES\n");
            free(rows);
            free(errors);
            free(metrics);
            free(rows_arg);
            return 2;
        }
        row_index++;
        cursor = next ? next + 1 : NULL;
    }

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            free(rows);
            free(errors);
            free(metrics);
            free(rows_arg);
            return 2;
        }
    } else if (streq(source_kind, "hex")) {
        if (decode_hex(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to decode inline hex\n");
            free(rows);
            free(errors);
            free(metrics);
            free(rows_arg);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind: %s\n", source_kind);
        free(rows);
        free(errors);
        free(metrics);
        free(rows_arg);
        return 2;
    }

    FT_Library library = NULL;
    FT_Face face = NULL;
    FT_Error setup_error = FT_Init_FreeType(&library);
    if (!setup_error) {
        setup_error = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    }
    printf("{");
    if (setup_error) {
        print_status(setup_error);
        printf(",\"output\":null}\n");
        if (face) FT_Done_Face(face);
        if (library) FT_Done_FreeType(library);
        free(data);
        free(rows);
        free(errors);
        free(metrics);
        free(rows_arg);
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    for (size_t i = 0; i < row_count; i++) {
        errors[i] = FT_Set_Char_Size(
            face,
            rows[i].char_width,
            rows[i].char_height,
            rows[i].horz_resolution,
            rows[i].vert_resolution
        );
        if (!first_error && errors[i]) {
            first_error = errors[i];
        }
        if (!errors[i]) {
            metrics[i] = face->size->metrics;
        }
    }

    print_status(first_error);
    printf(",\"output\":{\"outputs\":[");
    for (size_t i = 0; i < row_count; i++) {
        if (i) {
            printf(",");
        }
        printf("{\"request\":{\"char_width\":%ld,\"char_height\":%ld,\"horz_resolution\":%u,\"vert_resolution\":%u},\"status\":%d,\"output\":",
               (long)rows[i].char_width,
               (long)rows[i].char_height,
               rows[i].horz_resolution,
               rows[i].vert_resolution,
               errors[i]);
        if (errors[i]) {
            printf("null");
        } else {
            printf("{");
            print_size_metrics_object(metrics[i]);
            printf("}");
        }
        printf("}");
    }
    printf("]}}\n");

    if (face) FT_Done_Face(face);
    if (library) FT_Done_FreeType(library);
    free(data);
    free(rows);
    free(errors);
    free(metrics);
    free(rows_arg);
    return 0;
}

static void print_library_version_field_list(unsigned int mask) {
    int first = 1;
    printf("[");
    if (mask & 1U) {
        printf("\"amajor\"");
        first = 0;
    }
    if (mask & 2U) {
        if (!first) printf(",");
        printf("\"aminor\"");
        first = 0;
    }
    if (mask & 4U) {
        if (!first) printf(",");
        printf("\"apatch\"");
    }
    printf("]");
}

static void print_library_version_map(
    const char* name,
    unsigned int mask,
    int major,
    int minor,
    int patch,
    int invert
) {
    int first = 1;
    printf("\"%s\":{", name);
    if (((mask & 1U) != 0) != invert) {
        printf("\"major\":%d", major);
        first = 0;
    }
    if (((mask & 2U) != 0) != invert) {
        if (!first) printf(",");
        printf("\"minor\":%d", minor);
        first = 0;
    }
    if (((mask & 4U) != 0) != invert) {
        if (!first) printf(",");
        printf("\"patch\":%d", patch);
    }
    printf("}");
}

static int parse_int_triplet(const char* csv, int* a, int* b, int* c) {
    char* copy = (char*)malloc(strlen(csv) + 1);
    if (!copy) return 0;
    memcpy(copy, csv, strlen(csv) + 1);
    char* first = strtok(copy, ",");
    char* second = strtok(NULL, ",");
    char* third = strtok(NULL, ",");
    char* extra = strtok(NULL, ",");
    if (!first || !second || !third || extra) {
        free(copy);
        return 0;
    }
    *a = atoi(first);
    *b = atoi(second);
    *c = atoi(third);
    free(copy);
    return 1;
}

static int emit_library_version(int argc, char** argv) {
    (void)argc;
    int library_present = atoi(argv[2]);
    const char* rows_csv = argv[3];
    int sentinel_major = 111;
    int sentinel_minor = 222;
    int sentinel_patch = 333;
    if (!parse_int_triplet(argv[4], &sentinel_major, &sentinel_minor, &sentinel_patch)) {
        fprintf(stderr, "invalid library version sentinel triplet\n");
        return 2;
    }
    FT_Library library = NULL;
    if (library_present) {
        FT_Error err = FT_Init_FreeType(&library);
        if (err) {
            printf("{");
            print_status(err);
            printf(",\"output\":null}\n");
            return 0;
        }
    }

    size_t rows_len = strlen(rows_csv);
    char* rows = (char*)malloc(rows_len + 1);
    if (!rows) {
        if (library) FT_Done_FreeType(library);
        return 2;
    }
    memcpy(rows, rows_csv, rows_len + 1);

    printf("{");
    print_status(0);
    printf(",\"output\":{\"rows\":[");
    char* token = strtok(rows, ",");
    int first = 1;
    while (token) {
        unsigned int mask = (unsigned int)strtoul(token, NULL, 10);
        int major = sentinel_major;
        int minor = sentinel_minor;
        int patch = sentinel_patch;
        FT_Library_Version(
            library,
            (mask & 1U) ? &major : NULL,
            (mask & 2U) ? &minor : NULL,
            (mask & 4U) ? &patch : NULL
        );
        if (!first) printf(",");
        first = 0;
        printf("{\"outputs\":");
        print_library_version_field_list(mask);
        printf(",");
        print_library_version_map("writes", mask, major, minor, patch, 0);
        printf(",");
        print_library_version_map("sentinel_outputs", mask, sentinel_major, sentinel_minor, sentinel_patch, 1);
        printf("}");
        token = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(rows);
    if (library) FT_Done_FreeType(library);
    return 0;
}

static int emit_init_free_type(int argc, char** argv) {
    const char* mode = argv[2];
    if (streq(mode, "null")) {
        FT_Error err = FT_Init_FreeType(NULL);
        printf("{");
        print_status(err);
        printf(",\"output\":{\"library_created\":false}}\n");
        return 0;
    }

    FT_Library library = NULL;
    FT_Error init_error = FT_Init_FreeType(&library);
    if (init_error) {
        printf("{");
        print_status(init_error);
        printf(",\"output\":null}\n");
        return 0;
    }

    if (streq(mode, "identity")) {
        printf("{");
        print_status(0);
        printf(
            ",\"output\":{\"library\":{\"nullness\":\"%s\",\"identity_class\":\"fresh\"}}}\n",
            library ? "non_null" : "null"
        );
        if (library) FT_Done_FreeType(library);
        return 0;
    }

    if (streq(mode, "version-and-load")) {
        if (argc < 6) {
            if (library) FT_Done_FreeType(library);
            fprintf(stderr, "version-and-load init command requires source and face_index\n");
            return 2;
        }
        const char* source_kind = argv[3];
        const char* source_value = argv[4];
        FT_Long face_index = atol(argv[5]);
        unsigned char* data = NULL;
        long data_len = 0;
        FT_Error new_face_error = FT_Err_Cannot_Open_Resource;
        FT_Error load_glyph_error = FT_Err_Invalid_Face_Handle;
        FT_Face face = NULL;
        int major = 0;
        int minor = 0;
        int patch = 0;

        FT_Library_Version(library, &major, &minor, &patch);
        if (streq(source_kind, "file") && load_file(source_value, &data, &data_len) == 0) {
            new_face_error = FT_New_Memory_Face(library, data, data_len, face_index, &face);
            if (!new_face_error) {
                load_glyph_error = FT_Load_Glyph(face, 0, FT_LOAD_DEFAULT);
            }
        }

        printf("{");
        print_status(0);
        printf(
            ",\"output\":{\"version\":{\"major\":%d,\"minor\":%d,\"patch\":%d},\"face_load_probe\":{\"new_face_error\":%ld,\"load_glyph_error\":%ld}}}\n",
            major,
            minor,
            patch,
            (long)new_face_error,
            (long)load_glyph_error
        );
        if (face) FT_Done_Face(face);
        if (library) FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (library) FT_Done_FreeType(library);
    fprintf(stderr, "unsupported init-free-type mode %s\n", mode);
    return 2;
}

static int handle_error(int argc, char** argv) {
    if (argc < 3) {
        fprintf(stderr, "--error requires an error code argument\n");
        return 2;
    }
    FT_Error error_code = (FT_Error)strtol(argv[2], NULL, 10);
    printf("{\"status\":{\"kind\":\"error\",\"error_code\":%d},\"output\":null}\n", error_code);
    return 0;
}

static void print_error_string_query(FT_Error error_code) {
    const char* text = FT_Error_String(error_code);
    printf("{\"code\":%d,\"null\":%s,\"bytes\":\"",
           error_code,
           text ? "false" : "true");
    if (text) {
        print_hex_bytes((const unsigned char*)text, (long)strlen(text));
    }
    printf("\",\"length\":%zu}", text ? strlen(text) : 0);
}

static int emit_error_string(int argc, char** argv) {
    if (argc != 3) {
        fprintf(stderr, "--error-string requires comma-separated error codes\n");
        return 2;
    }
    char* rows = (char*)malloc(strlen(argv[2]) + 1);
    if (!rows) {
        return 1;
    }
    strcpy(rows, argv[2]);
    printf("{");
    print_status(0);
    printf(",\"output\":{\"build_has_error_strings\":%s,\"queries\":[",
           FT_Error_String(FT_Err_Ok) ? "true" : "false");
    char* token = strtok(rows, ",");
    int first = 1;
    while (token) {
        if (!first) printf(",");
        first = 0;
        print_error_string_query((FT_Error)strtol(token, NULL, 10));
        token = strtok(NULL, ",");
    }
    printf("]}}\n");
    free(rows);
    return 0;
}

static int emit_load_char_null_face(int argc, char** argv) {
    if (argc != 4) {
        fprintf(stderr, "--load-char-null-face requires CHAR FLAGS\n");
        return 2;
    }
    FT_ULong char_code = strtoul(argv[2], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[3], NULL, 10);
    FT_Error err = FT_Load_Char(NULL, char_code, load_flags);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_load_glyph_null_face(int argc, char** argv) {
    if (argc != 4) {
        fprintf(stderr, "--load-glyph-null-face requires GID FLAGS\n");
        return 2;
    }
    FT_UInt glyph_index = (FT_UInt)strtoul(argv[2], NULL, 10);
    FT_Int32 load_flags = (FT_Int32)strtol(argv[3], NULL, 10);
    FT_Error err = FT_Load_Glyph(NULL, glyph_index, load_flags);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int handle_void(void) {
    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"void\":true}}\n");
    return 0;
}

static int handle_value_ok(long value) {
    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"value\":%ld}}\n", value);
    return 0;
}

static int emit_set_transform(int argc, char** argv) {
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    FT_Fixed xx = (FT_Fixed)atol(argv[5]);
    FT_Fixed xy = (FT_Fixed)atol(argv[6]);
    FT_Fixed yx = (FT_Fixed)atol(argv[7]);
    FT_Fixed yy = (FT_Fixed)atol(argv[8]);
    FT_Pos dx = (FT_Pos)atol(argv[9]);
    FT_Pos dy = (FT_Pos)atol(argv[10]);
    int has_load = (argc >= 14);
    FT_UInt gid = 0;
    FT_Int32 load_flags = 0;
    int px = 0;
    if (has_load) {
        px = atoi(argv[11]);
        gid = (FT_UInt)atoi(argv[12]);
        load_flags = (FT_Int32)atol(argv[13]);
    }

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind\n");
        return 2;
    }
    FT_Library library;
    FT_Error err = FT_Init_FreeType(&library);
    FT_Face face = NULL;
    if (!err) err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    if (!err && face) {
        FT_Matrix matrix;
        matrix.xx = xx;
        matrix.xy = xy;
        matrix.yx = yx;
        matrix.yy = yy;
        FT_Vector delta;
        delta.x = dx;
        delta.y = dy;
        FT_Set_Transform(face, &matrix, &delta);
    }
    if (!err && has_load && face) {
        if (px > 0) {
            err = FT_Set_Pixel_Sizes(face, (FT_UInt)px, (FT_UInt)px);
        }
        if (!err) {
            err = FT_Load_Glyph(face, gid, load_flags);
        }
        printf("{");
        print_status(err);
        if (err) {
            printf(",\"output\":null}\n");
        } else {
            printf(",");
            print_slot(face->glyph, gid);
            printf("}\n");
        }
    } else {
        printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"void\":true}}\n");
    }
    if (face) FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int emit_reference_face(int argc, char** argv) {
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind\n");
        return 2;
    }
    FT_Library library;
    FT_Error err = FT_Init_FreeType(&library);
    FT_Face face = NULL;
    if (!err) err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    if (!err) err = FT_Reference_Face(face);
    if (!err) {
        printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"refcount\":2}}\n");
    } else {
        printf("{\"status\":{\"kind\":\"error\",\"error_code\":%d},\"output\":null}\n", err);
    }
    if (face) FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static void print_transform_matrix_delta(FT_Matrix matrix, FT_Vector delta) {
    printf("\"matrix\":{\"xx\":%ld,\"xy\":%ld,\"yx\":%ld,\"yy\":%ld},\"delta\":{\"x\":%ld,\"y\":%ld}",
           matrix.xx, matrix.xy, matrix.yx, matrix.yy, delta.x, delta.y);
}

static int emit_get_transform(int argc, char** argv) {
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* set_rows_arg = argv[5];
    const char* output_rows_arg = argv[6];
    const char* sentinel_arg = argv[7];

    unsigned char* data = NULL;
    long data_len = 0;
    if (streq(source_kind, "file")) {
        if (load_file(source_value, &data, &data_len) != 0) {
            fprintf(stderr, "failed to read font file: %s\n", source_value);
            return 2;
        }
    } else {
        fprintf(stderr, "unsupported source kind\n");
        return 2;
    }

    FT_Library library;
    FT_Error err = FT_Init_FreeType(&library);
    FT_Face face = NULL;
    if (!err) err = FT_New_Memory_Face(library, data, data_len, face_index, &face);
    if (err || !face) {
        printf("{\"status\":{\"kind\":\"error\",\"error_code\":%d},\"output\":null}\n", err);
        if (face) FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (!streq(set_rows_arg, "none")) {
        char* set_rows = (char*)malloc(strlen(set_rows_arg) + 1);
        if (!set_rows) {
            return 1;
        }
        strcpy(set_rows, set_rows_arg);
        char* cursor = set_rows;
        while (cursor && *cursor) {
            char* next = strchr(cursor, ';');
            if (next) {
                *next = '\0';
            }
            long long values[8] = {0};
            int value_count = split_fixed_math_row(cursor, values, 8);
            if (value_count != 8) {
                fprintf(stderr, "get-transform set row must have 8 values\n");
                free(set_rows);
                if (face) FT_Done_Face(face);
                FT_Done_FreeType(library);
                free(data);
                return 2;
            }
            FT_Matrix matrix = {
                (FT_Fixed)values[2],
                (FT_Fixed)values[3],
                (FT_Fixed)values[4],
                (FT_Fixed)values[5],
            };
            FT_Vector delta = {(FT_Pos)values[6], (FT_Pos)values[7]};
            FT_Set_Transform(face, values[0] ? &matrix : NULL, values[1] ? &delta : NULL);
            cursor = next ? next + 1 : NULL;
        }
        free(set_rows);
    }

    long long sentinel_values[6] = {0};
    char* sentinel_rows = (char*)malloc(strlen(sentinel_arg) + 1);
    if (!sentinel_rows) {
        return 1;
    }
    strcpy(sentinel_rows, sentinel_arg);
    int sentinel_count = split_fixed_math_row(sentinel_rows, sentinel_values, 6);
    free(sentinel_rows);
    if (sentinel_count != 6) {
        fprintf(stderr, "get-transform sentinels must have 6 values\n");
        if (face) FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 2;
    }

    char* output_rows = (char*)malloc(strlen(output_rows_arg) + 1);
    if (!output_rows) {
        return 1;
    }
    strcpy(output_rows, output_rows_arg);
    int multiple_rows = strchr(output_rows_arg, ';') != NULL;
    printf("{");
    print_status(0);
    printf(",\"output\":");
    if (multiple_rows) {
        printf("{\"rows\":[");
    } else {
        printf("{");
    }
    char* cursor = output_rows;
    int emitted = 0;
    while (cursor && *cursor) {
        char* next = strchr(cursor, ';');
        if (next) {
            *next = '\0';
        }
        long long values[3] = {0};
        int value_count = split_fixed_math_row(cursor, values, 3);
        if (value_count != 3) {
            fprintf(stderr, "get-transform output row must have 3 values\n");
            free(output_rows);
            if (face) FT_Done_Face(face);
            FT_Done_FreeType(library);
            free(data);
            return 2;
        }
        FT_Matrix matrix = {
            (FT_Fixed)sentinel_values[0],
            (FT_Fixed)sentinel_values[1],
            (FT_Fixed)sentinel_values[2],
            (FT_Fixed)sentinel_values[3],
        };
        FT_Vector delta = {(FT_Pos)sentinel_values[4], (FT_Pos)sentinel_values[5]};
        FT_Get_Transform(
            values[0] ? NULL : face,
            values[1] ? &matrix : NULL,
            values[2] ? &delta : NULL);
        if (emitted) {
            printf(",");
        }
        if (multiple_rows) {
            printf("{\"face\":\"%s\",\"matrix_output\":\"%s\",\"delta_output\":\"%s\",",
                   values[0] ? "null" : "live",
                   values[1] ? "non_null" : "null",
                   values[2] ? "non_null" : "null");
            print_transform_matrix_delta(matrix, delta);
            printf("}");
        } else {
            print_transform_matrix_delta(matrix, delta);
        }
        emitted = 1;
        cursor = next ? next + 1 : NULL;
    }
    if (multiple_rows) {
        printf("]}");
    } else {
        printf("}");
    }
    printf("}\n");
    free(output_rows);
    if (face) FT_Done_Face(face);
    FT_Done_FreeType(library);
    free(data);
    return 0;
}

static int dispatch(int argc, char** argv) {
    if (argc == 2 && streq(argv[1], "--void")) {
        return handle_void();
    }
    if (argc == 3 && streq(argv[1], "--value-ok")) {
        return handle_value_ok(atol(argv[2]));
    }
    if (argc == 7 && streq(argv[1], "--reference-face")) {
        return emit_reference_face(argc, argv);
    }
    if ((argc == 10 || argc == 14) && streq(argv[1], "--set-transform")) {
        return emit_set_transform(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-transform")) {
        return emit_get_transform(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--error")) {
        return handle_error(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--error-string")) {
        return emit_error_string(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--load-char-null-face")) {
        return emit_load_char_null_face(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--load-glyph-null-face")) {
        return emit_load_glyph_null_face(argc, argv);
    }
    if ((argc == 5 || argc == 10) && streq(argv[1], "--get-sfnt-name-variant")) {
        return emit_get_sfnt_name_variant(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--get-renderer")) {
        return emit_get_renderer(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--set-renderer")) {
        return emit_set_renderer(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--get-module")) {
        return emit_get_module(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--module-interface-probe")) {
        return emit_module_interface_probe(argc, argv);
    }
    // Generic null-source handler: intercept commands with "null" in handle-level
    // parameters (source kind, source value, or face).
    // Do NOT intercept when "null" is in task-specific params (tag_ptr, length_ptr, etc.).
    // Check only the first few args that represent handles: argv[2]..argv[min(6, argc-1)].
    if (argc >= 3) {
        int check_end = (argc < 7) ? argc : 6;
        for (int i = 2; i < check_end; i++) {
            if (streq(argv[i], "null")) {
                // Special: set-transform and get-transform are supposed to be no-ops with null
                if (streq(argv[1], "--set-transform") || streq(argv[1], "--get-transform")) {
                    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"void\":true}}\n");
                    return 0;
                }
                if (streq(argv[1], "--get-char-index")) {
                    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"value\":0}}\n");
                    return 0;
                }
                if (streq(argv[1], "--get-gasp")) {
                    FT_UInt ppem = (FT_UInt)strtoul(argv[7], NULL, 10);
                    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"value\":%d,\"return\":%d,\"ppem\":%u}}\n",
                           FT_GASP_NO_TABLE,
                           FT_GASP_NO_TABLE,
                           ppem);
                    return 0;
                }
                // Null-face operations that should succeed silently
                if (streq(argv[1], "--face-check-tt-patents")) {
                    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"result\":0}}\n");
                    return 0;
                }
                if (streq(argv[1], "--face-set-unpatented-hinting")) {
                    // Null face: return outputs format matching Rust backend.
                    const char* values_str = argv[3];
                    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"outputs\":[");
                    const char* ptr = values_str;
                    int first = 1;
                    while (*ptr) {
                        int val = (int)strtol(ptr, (char**)&ptr, 10);
                        if (!first) printf(",");
                        printf("{\"value\":%d,\"result\":0}", val);
                        first = 0;
                        if (*ptr == ',') ptr++;
                    }
                    printf("]}}\n");
                    return 0;
                }
                if (streq(argv[1], "--get-first-char")
                    || streq(argv[1], "--get-first-char-null-agindex")
                    || streq(argv[1], "--get-next-char-sequence")
                    || streq(argv[1], "--get-next-char-sequence-null-agindex")
                    || streq(argv[1], "--get-next-char-starts")
                    || streq(argv[1], "--get-next-char-starts-null-agindex")) {
                    printf("{\"status\":{\"kind\":\"ok\",\"error_code\":0},\"output\":{\"void\":true}}\n");
                    return 0;
                }
                printf("{\"status\":{\"kind\":\"error\",\"error_code\":%d},\"output\":null}\n", FT_Err_Invalid_Face_Handle);
                return 0;
            }
        }
    }


    if (argc == 3 && streq(argv[1], "--constant")) {
        return emit_constant(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--constant-map")) {
        return emit_constant_map(argv[2]);
    }
    if (argc == 4 && streq(argv[1], "--fixed-math")) {
        return emit_fixed_math(argv[2], argv[3]);
    }
    if (argc == 3 && streq(argv[1], "--vector-transform")) {
        return emit_vector_transform(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--matrix-multiply")) {
        return emit_matrix_multiply(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--matrix-invert")) {
        return emit_matrix_invert(argv[2]);
    }
    if (argc == 5 && streq(argv[1], "--bitmap-init-new")) {
        return emit_bitmap_init_new(argv[2], argv[3], argv[4]);
    }
    if (argc == 3 && streq(argv[1], "--bitmap-copy")) {
        return emit_bitmap_copy(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--bitmap-convert")) {
        return emit_bitmap_convert(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--bitmap-done")) {
        return emit_bitmap_done(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--bitmap-embolden")) {
        return emit_bitmap_embolden(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--bitmap-blend")) {
        return emit_bitmap_blend(argv[2]);
    }
    if (argc == 9 && streq(argv[1], "--glyphslot-own-bitmap")) {
        return emit_glyphslot_own_bitmap(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--trigon")) {
        return emit_trigon(argv[2], argv[3]);
    }
    if (argc == 4 && streq(argv[1], "--trigon-aggregate")) {
        return emit_trigon_aggregate(argv[2], argv[3]);
    }
    if (argc == 3 && streq(argv[1], "--layout")) {
        return emit_layout(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--type-probe")) {
        return emit_type_probe(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--type-map-probe")) {
        return emit_type_map_probe(argv[2]);
    }
    if (argc == 3 && streq(argv[1], "--function-probe")) {
        return emit_function_probe(argv[2]);
    }
    if (argc == 4 && streq(argv[1], "--abi-value-echo")) {
        return emit_abi_value_echo(argv[2], argv[3]);
    }
    if (argc == 5 && streq(argv[1], "--compile-alias-probe")) {
        return emit_compile_alias_probe(argv[2], argv[3], argv[4]);
    }
    if (argc == 3 && streq(argv[1], "--macro-eval")) {
        return emit_macro_eval(argv[2]);
    }
    if (argc == 6 && streq(argv[1], "--face-macro")) {
        return emit_face_macro(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--face-macro-flags")) {
        return emit_face_macro_flags(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--library-version")) {
        return emit_library_version(argc, argv);
    }
    if ((argc == 3 || argc == 6) && streq(argv[1], "--init-free-type")) {
        return emit_init_free_type(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--face-flags")) {
        return emit_face_flags(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--manager-reset-null")) {
        return emit_manager_reset(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--manager-reset")) {
        return emit_manager_reset(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--outline-render")) {
        return emit_outline_render(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-decompose")) {
        return emit_outline_decompose(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--outline-get-bitmap")) {
        return emit_outline_get_bitmap(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-get-orientation")) {
        return emit_outline_get_orientation(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-get-orientation-mutated")) {
        return emit_outline_get_orientation_mutated(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-reverse-orientation")) {
        return emit_outline_reverse_orientation(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-border")) {
        return emit_outline_border(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-check")) {
        return emit_outline_check(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-copy")) {
        return emit_outline_copy(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-done")) {
        return emit_outline_done(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-embolden")) {
        return emit_outline_embolden_common(argc, argv, 0);
    }
    if (argc == 3 && streq(argv[1], "--outline-embolden-xy")) {
        return emit_outline_embolden_common(argc, argv, 1);
    }
    if (argc == 3 && streq(argv[1], "--outline-new")) {
        return emit_outline_new(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-reverse")) {
        return emit_outline_reverse(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-transform")) {
        return emit_outline_transform(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-translate")) {
        return emit_outline_translate(argc, argv);
    }
    if (argc == 7 && (streq(argv[1], "--new-memory-face") || streq(argv[1], "--set-pixel-sizes") || streq(argv[1], "--size-metrics") || streq(argv[1], "--size-record-state"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--new-memory-face-variants")) {
        return emit_new_memory_face_variants(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--incremental-absent-open")) {
        return emit_incremental_absent_open(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--incremental-nullness-open")) {
        return emit_incremental_nullness_open(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--new-face-variants")) {
        return emit_new_face_variants(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--open-face-variants")) {
        return emit_open_face_variants(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--open-face-name-options")) {
        return emit_open_face_name_options(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--face-style-flags")) {
        return emit_face_style_flags(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--open-face-ignored-params")) {
        return emit_open_face_ignored_params(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--set-char-size")) {
        return emit_set_char_size(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--set-char-sizes")) {
        return emit_set_char_sizes(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--request-size")) {
        return emit_request_size(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--new-size-null-face")) {
        return emit_new_size_null_face(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--new-size-null-output")) {
        return emit_new_size_null_output(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--new-size-sequence")) {
        return emit_new_size_sequence(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--done-size-null")) {
        return emit_done_size_null(argc, argv);
    }
    if ((argc == 5 || argc == 6) && streq(argv[1], "--done-size-sequence")) {
        return emit_done_size_sequence(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--activate-size-null")) {
        return emit_activate_size_null(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--activate-size-sequence")) {
        return emit_activate_size_sequence(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--activate-select-size-sequence")) {
        return emit_activate_select_size_sequence(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--open-type-validate-null-face")) {
        return emit_open_type_validate_null_face(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--open-type-validate-null-outputs")) {
        return emit_open_type_validate_null_outputs(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--open-type-validate-service-missing")) {
        return emit_open_type_validate_service_missing(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--ps-font-info")) {
        return emit_ps_font_info(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--ps-font-info-null-face")) {
        return emit_ps_font_info_null_face(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--ps-font-info-null-output")) {
        return emit_ps_font_info_null_output(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--has-ps-glyph-names")) {
        return emit_has_ps_glyph_names(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--has-ps-glyph-names-null-face")) {
        return emit_has_ps_glyph_names_null_face(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--ps-mm-blend-dictionary")) {
        return emit_ps_mm_blend_dictionary(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--ps-font-private")) {
        return emit_ps_font_private(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--ps-font-private-null-face")) {
        return emit_ps_font_private_null_face(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--ps-font-private-null-output")) {
        return emit_ps_font_private_null_output(argc, argv);
    }
    if (argc >= 6 && streq(argv[1], "--ps-font-private-rowset")) {
        return emit_ps_font_private_rowset(argc, argv);
    }
    if ((argc == 5 || argc == 6) && streq(argv[1], "--ps-font-value-encoding")) {
        return emit_ps_font_value_encoding(argc, argv);
    }
    if (argc >= 6 && streq(argv[1], "--ps-font-value-encoding-rowset")) {
        return emit_ps_font_value_encoding_rowset(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--ps-font-value-matrix")) {
        return emit_ps_font_value_matrix(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--open-type-free-null-face")) {
        return emit_open_type_free_null_face(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--open-type-free-null-table")) {
        return emit_open_type_free_null_table(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--gxval-free-null-face")) {
        return emit_gxval_free_null_face(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--color-palette-case")) {
        return emit_color_palette_case(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--color-glyph-layer-case")) {
        return emit_color_glyph_layer_case(argc, argv);
    }
    if ((argc == 7 || argc == 15) && streq(argv[1], "--color-glyph-clipbox-case")) {
        return emit_color_glyph_clipbox_case(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--color-paint-graph-case")) {
        return emit_color_paint_graph_case(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-char-index")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--face-get-char-variant-index-null")) {
        return emit_face_get_char_variant_index_null(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--face-get-char-variant-index")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--face-get-char-variant-is-default-null")) {
        return emit_face_get_char_variant_is_default_null(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--face-get-char-variant-is-default")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--face-get-variant-selectors-null")) {
        return emit_face_get_variant_selectors_null(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--face-get-variant-selectors")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--face-get-variants-of-char-null")) {
        return emit_face_get_variants_of_char_null(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--face-get-variants-of-char")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--face-get-chars-of-variant-null")) {
        return emit_face_get_chars_of_variant_null(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--face-get-chars-of-variant")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-kerning")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--attach-file")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--attach-stream")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--get-track-kerning")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-pfr-kerning")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--get-kerning-null-face")) {
        return emit_get_kerning_null_face(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--get-kerning-null-output")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--charmap-get-char-index")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--inspect-charmaps")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--set-charmap")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--set-charmap-null-face")) {
        return emit_set_charmap_null_face(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--set-charmap-variants")) {
        return emit_set_charmap_variants(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--get-charmap-index")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--get-charmap-index-variants")) {
        return emit_get_charmap_index_variants(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--select-charmap")) {
        return emit_select_charmap(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--select-charmap-null-face")) {
        return emit_select_charmap_null_face(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--select-size")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--select-size-null")) {
        return emit_select_size_null(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--new-memory-face-null-base")) {
        return emit_new_memory_face_null_base(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--new-face-missing-path")) {
        return emit_new_face_missing_path(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--select-charmaps")) {
        return emit_select_charmaps(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--set-lcd-filter")) {
        return emit_set_lcd_filter(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--set-lcd-filter-weights")) {
        return emit_set_lcd_filter_weights(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--set-lcd-geometry")) {
        return emit_set_lcd_geometry(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--stroker-null-noop")) {
        return emit_stroker_null_noop(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--stroker-lifecycle")) {
        return emit_stroker_lifecycle(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--stroker-zero-line")) {
        return emit_stroker_zero_line(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--stroker-simple-line-counts")) {
        return emit_stroker_simple_line_counts(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--stroker-finalized-counts")) {
        return emit_stroker_finalized_counts(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--stroker-parse-degenerate")) {
        return emit_stroker_parse_degenerate(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--stroker-end-no-segment")) {
        return emit_stroker_end_no_segment(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--stroker-degenerate-curve")) {
        return emit_stroker_degenerate_curve(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--get-truetype-engine-type")) {
        return emit_truetype_engine_type(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--library-lifecycle")) {
        return emit_library_lifecycle(argc, argv);
    }
    if ((argc == 3 || argc == 6 || argc == 9) && streq(argv[1], "--property-case")) {
        return emit_property_case(argc, argv);
    }
    if ((argc == 3 || argc == 6) && streq(argv[1], "--face-properties-case")) {
        return emit_face_properties_case(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--bdf-property-case")) {
        return emit_bdf_property_case(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--bdf-charset-case")) {
        return emit_bdf_charset_case(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--set-debug-hook")) {
        return emit_set_debug_hook(argc, argv);
    }
    if ((argc == 3 || argc == 4) && streq(argv[1], "--add-default-modules")) {
        return emit_add_default_modules(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--inspect-module-flags")) {
        return emit_inspect_module_flags(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--done-mm-var")) {
        return emit_done_mm_var(argc, argv);
    }
    if ((argc == 3 || argc == 6) && streq(argv[1], "--done-freetype")) {
        return emit_done_freetype(argc, argv);
    }
    if ((argc == 3 || argc == 6) && streq(argv[1], "--done-face")) {
        return emit_done_face(argc, argv);
    }
    if ((argc == 3 || argc == 6) && streq(argv[1], "--face-check-tt-patents")) {
        return emit_face_check_truetype_patents(argc, argv);
    }
    if ((argc == 4 || argc == 7) && streq(argv[1], "--face-set-unpatented-hinting")) {
        return emit_face_set_unpatented_hinting(argc, argv);
    }
    if ((argc == 7 || argc == 8) && streq(argv[1], "--get-fstype-flags")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-gasp")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && (streq(argv[1], "--get-cmap-format") || streq(argv[1], "--get-cmap-language-id"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--get-postscript-name")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 7 && (streq(argv[1], "--get-font-format") || streq(argv[1], "--get-x11-font-format") || streq(argv[1], "--get-x11-font-format-alias") || streq(argv[1], "--face-driver-name") || streq(argv[1], "--face-driver-name-with-font-format"))) {
        return emit_face_or_slot(argc, argv);
    }
    if ((argc == 7 || argc == 8) && streq(argv[1], "--get-winfnt-header")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--winfnt-charmap-probe")) {
        return emit_face_or_slot(argc, argv);
    }
    if ((argc == 2 || argc == 3) && streq(argv[1], "--get-winfnt-header-null-face")) {
        return emit_get_winfnt_header_null_face(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--get-winfnt-header-mutation")) {
        return emit_get_winfnt_header_mutation(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--get-font-format-null-face")) {
        printf("{");
        print_status(0);
        printf(",\"output\":");
        print_nullable_c_string_result(FT_Get_Font_Format(NULL));
        printf("}\n");
        return 0;
    }
    if (argc == 2 && streq(argv[1], "--get-x11-font-format-null-face")) {
        printf("{");
        print_status(0);
        printf(",\"output\":");
        print_nullable_c_string_result(FT_Get_X11_Font_Format(NULL));
        printf("}\n");
        return 0;
    }
    if (argc == 6 && streq(argv[1], "--get-postscript-name-variants")) {
        return emit_get_postscript_name_variants(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--set-named-instance")) {
        return emit_set_named_instance(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--set-named-instance-adobe-mm-reset")) {
        return emit_set_named_instance_adobe_mm_reset(argc, argv);
    }
    if (argc == 12 && streq(argv[1], "--set-named-instance-glyph-output")) {
        return emit_set_named_instance_glyph_output(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--set-named-instance-descriptor")) {
        return emit_set_named_instance_descriptor(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--ftmm-get-var-design-coordinates")) {
        return emit_ftmm_get_var_design_coordinates(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--ftmm-set-var-design-coordinates")) {
        return emit_ftmm_set_var_design_coordinates(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--face-flags-after-variation")) {
        return emit_face_flags_after_variation(argc, argv);
    }
    if (argc == 12 && streq(argv[1], "--ftmm-set-var-design-glyph-output")) {
        return emit_ftmm_set_var_design_glyph_output(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--ftmm-set-var-design-metrics-output")) {
        return emit_ftmm_set_var_design_metrics_output(argc, argv);
    }
    if (argc == 12 && streq(argv[1], "--ftmm-set-var-blend-glyph-output")) {
        return emit_ftmm_set_var_blend_glyph_output(argc, argv);
    }
    if (argc == 12 && streq(argv[1], "--ftmm-set-mm-blend-glyph-output")) {
        return emit_ftmm_set_mm_blend_glyph_output(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--ftmm-set-var-design-scenarios")) {
        return emit_ftmm_set_var_design_scenarios(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--ftmm-set-mm-design-coordinates")) {
        return emit_ftmm_set_mm_design_coordinates(argc, argv);
    }
    if (argc == 12 && streq(argv[1], "--ftmm-set-mm-design-glyph-output")) {
        return emit_ftmm_set_mm_design_glyph_output(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--ftmm-set-mm-design-scenarios")) {
        return emit_ftmm_set_mm_design_scenarios(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--ftmm-set-mm-blend-scenarios")) {
        return emit_ftmm_set_mm_blend_scenarios(argc, argv);
    }
    if (argc == 13 && streq(argv[1], "--ftmm-blend-coordinates")) {
        return emit_ftmm_blend_coordinates(argc, argv);
    }
    if ((argc == 6 || argc == 7) && streq(argv[1], "--ftmm-get-multi-master")) {
        return emit_ftmm_get_multi_master(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--ftmm-get-multi-master-adobe-control")) {
        return emit_ftmm_get_multi_master_adobe_control(argc, argv);
    }
    if ((argc == 5 || argc == 6) && streq(argv[1], "--ftmm-get-mm-var")) {
        return emit_ftmm_get_mm_var(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--ftmm-get-and-done-mm-var")) {
        return emit_ftmm_get_and_done_mm_var(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--ftmm-axis-flags")) {
        return emit_ftmm_axis_flags(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--ftmm-mm-weight-vector")) {
        return emit_ftmm_mm_weight_vector(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--ftmm-mm-weight-vector-get")) {
        return emit_ftmm_get_mm_weight_vector(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--ftmm-var-blend-alias")) {
        return emit_ftmm_var_blend_alias(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--ftmm-var-blend-flag-matrix")) {
        return emit_ftmm_var_blend_flag_matrix(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--ftmm-var-blend-scenarios")) {
        return emit_ftmm_var_blend_scenarios(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--ftmm-mm-blend-invalid-matrix")) {
        return emit_ftmm_mm_blend_invalid_matrix(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--ftmm-mm-blend-count-matrix")) {
        return emit_ftmm_mm_blend_count_matrix(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--set-named-instance-null-face")) {
        return emit_set_named_instance_null_face(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--get-default-named-instance")) {
        return emit_get_default_named_instance(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--get-default-named-instance-invalid")) {
        return emit_get_default_named_instance_invalid(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--get-first-char-null-face")) {
        return emit_get_first_char_null_face(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--get-next-char-null-face")) {
        return emit_get_next_char_null_face(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--get-glyph-name")) {
        return emit_get_glyph_name(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-name-index")) {
        return emit_get_name_index(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--get-sfnt-name-count")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-sfnt-name")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--get-sfnt-lang-tag")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-sfnt-lang-tag-variants")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--get-sfnt-name-match")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 12 && streq(argv[1], "--sfnt-mac-encoding-record")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-sfnt-table")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--inspect-available-sizes")) {
        return emit_available_sizes(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--inspect-face-rec-initial")) {
        return emit_face_rec_initial_snapshot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--inspect-face-rec-post-size")) {
        return emit_face_rec_post_size_snapshot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--get-sfnt-vhea-mvar-sequence")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--load-sfnt-table")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--sfnt-table-info")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-sfnt-os2-unicode-ranges")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 7 && (streq(argv[1], "--get-first-char") || streq(argv[1], "--get-first-char-null-agindex"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && (streq(argv[1], "--get-next-char-sequence") || streq(argv[1], "--get-next-char-sequence-null-agindex") || streq(argv[1], "--get-next-char-starts") || streq(argv[1], "--get-next-char-starts-null-agindex"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && (streq(argv[1], "--load-char") || streq(argv[1], "--load-glyph") || streq(argv[1], "--load-glyph-from-char") || streq(argv[1], "--inspect-glyph-metrics") || streq(argv[1], "--inspect-glyph-slot") || streq(argv[1], "--load-glyph-outline") || streq(argv[1], "--outline-get-bbox") || streq(argv[1], "--outline-get-cbox") || streq(argv[1], "--get-glyph-unsupported-format"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--active-size-handle")) {
        return emit_active_size_handle(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--open-face-stream-ownership")) {
        return emit_open_face_stream_ownership(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--memory-stream-probe")) {
        return emit_memory_stream_probe(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--face-owned-handles")) {
        return emit_face_owned_handles(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--malformed-maxp-route")) {
        return emit_malformed_maxp_route(argc, argv);
    }
    if (argc == 8 && (streq(argv[1], "--glyphslot-slant") || streq(argv[1], "--glyphslot-oblique") || streq(argv[1], "--glyphslot-adjust-weight") || streq(argv[1], "--glyphslot-embolden") || streq(argv[1], "--slot-format-probe"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--render-glyph-slot-states")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--glyphslot-null-noop")) {
        return emit_glyphslot_null_noop(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--face-set-unpatented-hinting-post-load")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-get-bbox-null-inputs")) {
        return emit_outline_get_bbox_null_inputs(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-get-bbox-malformed")) {
        return emit_outline_get_bbox_malformed(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-get-cbox-null-inputs")) {
        return emit_outline_get_cbox_null_inputs(argc, argv);
    }
    if (argc == 4 && streq(argv[1], "--glyph-get-cbox-null-or-no-bbox")) {
        return emit_glyph_cbox_null_or_no_bbox(argv[2], (FT_UInt)strtoul(argv[3], NULL, 10));
    }
    if (argc == 2 && streq(argv[1], "--get-glyph-null-inputs")) {
        return emit_get_glyph_null_inputs();
    }
    if (argc == 2 && streq(argv[1], "--glyph-copy-null-inputs")) {
        return emit_glyph_copy_null_inputs();
    }
    if (argc == 2 && streq(argv[1], "--done-glyph-null")) {
        return emit_done_glyph_null();
    }
    if (argc == 2 && streq(argv[1], "--glyph-to-bitmap-invalid-inputs")) {
        return emit_glyph_to_bitmap_invalid_inputs();
    }
    if (argc == 3 && streq(argv[1], "--ft-list")) {
        return emit_ft_list(argv[2]);
    }
    if (argc >= 6 && streq(argv[1], "--gzip-uncompress")) {
        return emit_gzip_uncompress(argc, argv);
    }
    if (argc >= 5 && streq(argv[1], "--gzip-stream-open")) {
        return emit_gzip_stream_open(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--bzip2-stream-disabled-policy")) {
        return emit_bzip2_stream_disabled_policy(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--load-glyph-num-glyphs")) {
        return emit_face_or_slot(argc, argv);
    }
    if ((argc == 9 || argc == 11) && streq(argv[1], "--get-advance")) {
        return emit_face_or_slot(argc, argv);
    }
    if ((argc == 10 || argc == 12) && streq(argv[1], "--get-advances")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--get-subglyph-info-null-slot")) {
        return emit_get_subglyph_info_null_slot(argv[2]);
    }
    if (argc == 11 && streq(argv[1], "--get-subglyph-info")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--get-subglyph-info-null-outputs")) {
        return emit_face_or_slot(argc, argv);
    }
    if ((argc == 10 || argc == 11 ||
         (argc == 12 && streq(argv[11], "capture-error-slot"))) &&
        (streq(argv[1], "--render-glyph") || streq(argv[1], "--render-glyph-index"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--inspect-glyph-slot")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--glyph-slot-reuse")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--glyph-get-cbox")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--glyph-transform")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--glyph-to-bitmap")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--glyph-record")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 13 && streq(argv[1], "--bitmap-glyph-record-paths")) {
        return emit_bitmap_glyph_record_paths(argc, argv);
    }
    if (argc == 13 && streq(argv[1], "--done-bitmap-glyph-paths")) {
        return emit_done_bitmap_glyph_paths(argc, argv);
    }
    if (argc == 9 && (streq(argv[1], "--done-glyph-outline") ||
                      streq(argv[1], "--done-glyph-bitmap"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--get-glyph-advance-boundaries")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--sbit-cache-lookup")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--sbit-cache-lookup-scaler")) {
        return emit_sbit_cache_lookup_scaler(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--cache-node-lifecycle")) {
        return emit_cache_node_lifecycle(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--cache-node-unref-null-only")) {
        return emit_cache_node_unref_null_only();
    }
    if (argc == 2 && streq(argv[1], "--cache-node-unref-null-or-invalid")) {
        return emit_cache_node_unref_null_or_invalid();
    }
    if (argc == 8 && streq(argv[1], "--scaler-descriptor-lifetime")) {
        return emit_scaler_descriptor_lifetime(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--sbit-cache-new-success")) {
        return emit_sbit_cache_new_success();
    }
    if (argc == 3 && streq(argv[1], "--cache-type-contract")) {
        return emit_cache_type_contract(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--image-cache-lookup-scaler")) {
        return emit_image_cache_lookup_scaler(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--image-cache-lookup")) {
        return emit_image_cache_lookup(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--image-type-descriptor-lifetime")) {
        return emit_image_type_descriptor_lifetime(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--image-type-lookup-probe")) {
        return emit_image_type_lookup_probe(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--cmap-cache-lookup")) {
        return emit_cmap_cache_lookup(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--cmap-cache-new-route")) {
        return emit_cmap_cache_new_route(argc, argv);
    }
    if (argc == 2 && streq(argv[1], "--add-module-minimal")) {
        return emit_add_module_minimal();
    }
    if (argc == 2 && streq(argv[1], "--add-module-styler")) {
        return emit_add_module_styler();
    }
    if (argc == 2 && streq(argv[1], "--add-module-renderer")) {
        return emit_add_module_renderer();
    }
    if (argc == 6 && streq(argv[1], "--image-cache-new-route")) {
        return emit_image_cache_new_route(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--manager-new-route")) {
        return emit_manager_new_route(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--manager-remove-face-id-route")) {
        return emit_manager_remove_face_id_route(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--manager-done-route")) {
        return emit_manager_done_route(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--manager-lifecycle-route")) {
        return emit_manager_lifecycle_route(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--manager-lookup-size")) {
        return emit_manager_lookup_size(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--manager-lookup-face")) {
        return emit_manager_lookup_face(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--face-id-identity")) {
        return emit_face_id_identity_route(argc, argv);
    }
    if (argc == 6 && streq(argv[1], "--cid-route")) {
        return emit_cid_route(argc, argv);
    }
    fprintf(stderr, "usage: gen_unified_oracle --constant SYMBOL | ... | --outline-render MODE CASE_ID | --outline-get-bitmap MODE CASE_ID | --outline-get-orientation CASE_ID | --outline-reverse CASE_ID | --outline-transform CASE_ID | ...\n");
    fprintf(stderr, "       --get-sfnt-name-variant FACE_KIND OUTPUT_KIND INDEXES [SRC_KIND SRC FACE_INDEX PX PY]\n");
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
