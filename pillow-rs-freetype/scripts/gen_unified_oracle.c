#include <ft2build.h>
#include FT_FREETYPE_H

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <freetype/ftcache.h>
#include <freetype/ftadvanc.h>
#include <freetype/ftbbox.h>
#include <freetype/ftbitmap.h>
#include <freetype/ftcolor.h>
#include <freetype/ftdriver.h>
#include <freetype/ftglyph.h>
#include <freetype/ftgasp.h>
#include <freetype/ftgxval.h>
#include <freetype/ftimage.h>
#include <freetype/ftincrem.h>
#include <freetype/ftlcdfil.h>
#include <freetype/ftmm.h>
#include <freetype/ftmodapi.h>
#include <freetype/ftoutln.h>
#include <freetype/ftrender.h>
#include <freetype/ftsnames.h>
#include <freetype/ftsystem.h>
#include <freetype/ftsizes.h>
#include <freetype/ftsynth.h>
#include <freetype/ftotval.h>
#include <freetype/fttrigon.h>
#include <freetype/t1tables.h>
#include <freetype/tttables.h>

#ifndef FT_ERR_PREFIX
#define FT_ERR_PREFIX FT_Err_
#endif

static int streq(const char* a, const char* b) {
    return strcmp(a, b) == 0;
}

static void print_json_bool(int value);

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

static void print_glyph_record_payload(FT_Glyph glyph) {
    printf("\"output\":{\"glyph\":{");
    printf("\"format\":%ld,", (long)glyph->format);
    printf("\"advance\":{\"x\":%ld,\"y\":%ld},", glyph->advance.x, glyph->advance.y);
    printf("\"library_present\":%s,", glyph->library ? "true" : "false");
    printf("\"clazz_present\":%s", glyph->clazz ? "true" : "false");
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

static int emit_outline_render(int argc, char** argv) {
    (void)argc;
    const char* mode = argv[2];
    if (streq(mode, "error")) {
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

    const char* case_id = argc > 3 ? argv[3] : "";
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
        strstr(case_id, "@even-odd-quad-wind")) {
        outline.flags = FT_OUTLINE_EVEN_ODD_FILL;
    }

    unsigned char buffer[32 * 32 + 256];
    memset(buffer, 0, sizeof(buffer));
    FT_Bitmap bitmap;
    memset(&bitmap, 0, sizeof(bitmap));
    bitmap.rows = bitmap_rows;
    bitmap.width = bitmap_width;
    bitmap.pitch = (int)bitmap_width;
    bitmap.buffer = buffer;
    bitmap.num_grays = 256;
    bitmap.pixel_mode = FT_PIXEL_MODE_GRAY;

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

    err = FT_Outline_Render(library, &outline, &params);
    printf("{");
    print_status(err);
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",");
        print_outline_render_bitmap_payload(&bitmap);
        printf("}\n");
    }
    FT_Done_FreeType(library);
    return 0;
}

typedef enum OrientationOutlineKind_ {
    ORIENTATION_OUTLINE_NULL,
    ORIENTATION_OUTLINE_EMPTY,
    ORIENTATION_OUTLINE_POSITIVE,
    ORIENTATION_OUTLINE_NEGATIVE,
    ORIENTATION_OUTLINE_COLLAPSED,
    ORIENTATION_OUTLINE_OVERSIZED,
    ORIENTATION_OUTLINE_ZERO_AREA,
} OrientationOutlineKind;

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
    if (kind == ORIENTATION_OUTLINE_COLLAPSED) {
        outline->n_points = 2;
        contours[0] = 1;
        points[0].x = 0;
        points[0].y = 0;
        points[1].x = 64;
        points[1].y = 0;
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
        print_orientation_observation("positive", ORIENTATION_OUTLINE_POSITIVE, &emitted);
        print_orientation_observation("negative", ORIENTATION_OUTLINE_NEGATIVE, &emitted);
    } else if (strstr(case_id, "FT_Outline_Get_Orientation.collapsed_and_oversized_return_none")) {
        print_orientation_observation("collapsed", ORIENTATION_OUTLINE_COLLAPSED, &emitted);
        print_orientation_observation("oversized", ORIENTATION_OUTLINE_OVERSIZED, &emitted);
    } else if (strstr(case_id, "FT_ORIENTATION_TRUETYPE.null_and_empty_return_truetype")) {
        print_orientation_observation("null", ORIENTATION_OUTLINE_NULL, &emitted);
        print_orientation_observation("empty", ORIENTATION_OUTLINE_EMPTY, &emitted);
    } else if (strstr(case_id, "FT_ORIENTATION_TRUETYPE.negative_area_returns_truetype") ||
               strstr(case_id, "FT_ORIENTATION_FILL_RIGHT.alias_matches_truetype_orientation")) {
        print_orientation_observation("negative", ORIENTATION_OUTLINE_NEGATIVE, &emitted);
    } else if (strstr(case_id, "FT_ORIENTATION_POSTSCRIPT.positive_area_returns_postscript")) {
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
    if (err) {
        printf("{");
        print_status(err);
        printf(",\"output\":null}\n");
        close_oracle_face(out);
        return 1;
    }
    return 0;
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
    if (err) {
        printf(",\"output\":null}\n");
    } else {
        printf(",\"output\":{\"size_is_null\":");
        print_json_bool(size == NULL);
        printf("}}\n");
        if (size) {
            FT_Done_Size(size);
        }
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
    FT_Error err = FT_New_Size(face.face, NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    close_oracle_face(&face);
    return 0;
}

static int emit_done_size_null(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_Error err = FT_Done_Size(NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
    return 0;
}

static int emit_activate_size_null(int argc, char** argv) {
    (void)argc;
    (void)argv;
    FT_Error err = FT_Activate_Size(NULL);
    printf("{");
    print_status(err);
    printf(",\"output\":null}\n");
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

static int emit_truetype_engine_type(int argc, char** argv) {
    (void)argc;
    int library_present = atoi(argv[2]);
    FT_Library library = NULL;
    FT_Error err = FT_Err_Ok;
    if (library_present) {
        err = FT_Init_FreeType(&library);
    }

    printf("{");
    if (err) {
        print_status(err);
        printf(",\"output\":null}\n");
        return 0;
    }
    FT_TrueTypeEngineType engine_type = FT_Get_TrueType_Engine_Type(library);
    print_status(FT_Err_Ok);
    printf(",\"output\":{\"engine_type\":%d}}\n", engine_type);
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

static int emit_face_or_slot(int argc, char** argv) {
    const char* command = argv[1];
    const char* source_kind = argv[2];
    const char* source_value = argv[3];
    FT_Long face_index = atol(argv[4]);
    const char* size_arg = argv[5];
    FT_UInt pixel_width = (FT_UInt)strtoul(size_arg, NULL, 10);
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
    int preserve_probe_face =
        face_index < 0 &&
        (streq(command, "--get-advance") || streq(command, "--get-advances"));
    if (!err && !preserve_probe_face) {
        if (strncmp(size_arg, "char:", 5) == 0) {
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

    if (streq(command, "--get-first-char")) {
        FT_UInt glyph_index = 0;
        FT_ULong char_code = FT_Get_First_Char(face, &glyph_index);
        print_status(0);
        printf(",\"output\":");
        print_char_iteration_result(char_code, glyph_index);
        printf("}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-next-char-sequence")) {
        unsigned long max_steps = strtoul(argv[7], NULL, 10);
        FT_UInt glyph_index = 0;
        FT_ULong char_code = FT_Get_First_Char(face, &glyph_index);
        print_status(0);
        printf(",\"output\":{\"sequence\":[");
        for (unsigned long i = 0; i < max_steps && glyph_index != 0; i++) {
            if (i) printf(",");
            print_char_iteration_result(char_code, glyph_index);
            char_code = FT_Get_Next_Char(face, char_code, &glyph_index);
        }
        printf("]}}\n");
        FT_Done_Face(face);
        FT_Done_FreeType(library);
        free(data);
        return 0;
    }

    if (streq(command, "--get-next-char-starts")) {
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
            FT_ULong char_code = FT_Get_Next_Char(face, start, &glyph_index);
            if (!first) printf(",");
            first = 0;
            printf("{\"start\":%lu,\"result\":", (unsigned long)start);
            print_char_iteration_result(char_code, glyph_index);
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

    FT_UInt glyph_index = 0;
    FT_Int32 load_flags = 0;
    if (streq(command, "--load-char") || streq(command, "--render-glyph") || streq(command, "--load-glyph-from-char")) {
        FT_ULong char_code = strtoul(argv[7], NULL, 10);
        load_flags = (FT_Int32)strtol(argv[8], NULL, 10);
        glyph_index = FT_Get_Char_Index(face, char_code);
    } else if (streq(command, "--load-glyph-num-glyphs")) {
        glyph_index = (FT_UInt)face->num_glyphs;
        load_flags = (FT_Int32)strtol(argv[7], NULL, 10);
    } else if (streq(command, "--load-glyph") || streq(command, "--render-glyph-index") || streq(command, "--inspect-glyph-metrics") || streq(command, "--inspect-glyph-slot") || streq(command, "--load-glyph-outline") || streq(command, "--outline-get-bbox") || streq(command, "--outline-get-cbox") || streq(command, "--glyph-get-cbox") || streq(command, "--glyph-to-bitmap") || streq(command, "--glyph-record") || streq(command, "--sbit-cache-lookup") || streq(command, "--get-subglyph-info") || streq(command, "--get-subglyph-info-null-outputs")) {
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
        printf(",\"output\":null}\n");
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

typedef struct MemoryFaceRow_ {
    FT_Long face_index;
    int has_file_size;
    FT_Long file_size;
} MemoryFaceRow;

static int parse_memory_face_row(char* row, MemoryFaceRow* out) {
    char* fields[3];
    char* cursor = row;
    for (int i = 0; i < 3; i++) {
        fields[i] = cursor;
        char* sep = strchr(cursor, ':');
        if (i == 2) {
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
    return 1;
}

static void print_memory_face_row(MemoryFaceRow row, FT_Error err) {
    printf("{\"face_index\":%ld,\"file_size\":", row.face_index);
    if (row.has_file_size) {
        printf("%ld", row.file_size);
    } else {
        printf("null");
    }
    printf(",\"status\":%d,\"opened\":%s}", err, err ? "false" : "true");
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
    if ((!rows || !errors) && row_count > 0) {
        free(data);
        free(rows_arg);
        free(rows);
        free(errors);
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
        return 0;
    }

    FT_Error first_error = FT_Err_Ok;
    for (size_t i = 0; i < row_count; i++) {
        FT_Face face = NULL;
        FT_Long file_size = rows[i].has_file_size ? rows[i].file_size : data_len;
        errors[i] = FT_New_Memory_Face(library, data, file_size, rows[i].face_index, &face);
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
        print_memory_face_row(rows[i], errors[i]);
    }
    printf("]}}\n");

    if (library) {
        FT_Done_FreeType(library);
    }
    free(data);
    free(rows_arg);
    free(rows);
    free(errors);
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
                    || streq(argv[1], "--get-next-char-sequence")
                    || streq(argv[1], "--get-next-char-starts")) {
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
    if (argc == 3 && streq(argv[1], "--outline-get-orientation")) {
        return emit_outline_get_orientation(argc, argv);
    }
    if (argc == 7 && (streq(argv[1], "--new-memory-face") || streq(argv[1], "--set-pixel-sizes") || streq(argv[1], "--size-metrics"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--new-memory-face-variants")) {
        return emit_new_memory_face_variants(argc, argv);
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
    if (argc == 2 && streq(argv[1], "--open-type-free-null-face")) {
        return emit_open_type_free_null_face(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--open-type-free-null-table")) {
        return emit_open_type_free_null_table(argc, argv);
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
    if (argc == 3 && streq(argv[1], "--get-truetype-engine-type")) {
        return emit_truetype_engine_type(argc, argv);
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
    if (argc == 6 && streq(argv[1], "--get-postscript-name-variants")) {
        return emit_get_postscript_name_variants(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--set-named-instance")) {
        return emit_set_named_instance(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--set-named-instance-null-face")) {
        return emit_set_named_instance_null_face(argc, argv);
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
    if (argc == 11 && streq(argv[1], "--load-sfnt-table")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--sfnt-table-info")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--get-sfnt-os2-unicode-ranges")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 7 && streq(argv[1], "--get-first-char")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && (streq(argv[1], "--get-next-char-sequence") || streq(argv[1], "--get-next-char-starts"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && (streq(argv[1], "--load-char") || streq(argv[1], "--load-glyph") || streq(argv[1], "--load-glyph-from-char") || streq(argv[1], "--inspect-glyph-metrics") || streq(argv[1], "--inspect-glyph-slot") || streq(argv[1], "--load-glyph-outline") || streq(argv[1], "--outline-get-bbox") || streq(argv[1], "--outline-get-cbox"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 8 && (streq(argv[1], "--glyphslot-slant") || streq(argv[1], "--glyphslot-oblique") || streq(argv[1], "--glyphslot-adjust-weight") || streq(argv[1], "--glyphslot-embolden"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 5 && streq(argv[1], "--glyphslot-null-noop")) {
        return emit_glyphslot_null_noop(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--face-set-unpatented-hinting-post-load")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 3 && streq(argv[1], "--outline-get-cbox-null-inputs")) {
        return emit_outline_get_cbox_null_inputs(argc, argv);
    }
    if (argc == 8 && streq(argv[1], "--load-glyph-num-glyphs")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--get-advance")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--get-advances")) {
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
    if ((argc == 10 || argc == 11) && (streq(argv[1], "--render-glyph") || streq(argv[1], "--render-glyph-index"))) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--inspect-glyph-slot")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--glyph-get-cbox")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 11 && streq(argv[1], "--glyph-to-bitmap")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 10 && streq(argv[1], "--glyph-record")) {
        return emit_face_or_slot(argc, argv);
    }
    if (argc == 9 && streq(argv[1], "--sbit-cache-lookup")) {
        return emit_face_or_slot(argc, argv);
    }
    fprintf(stderr, "usage: gen_unified_oracle --constant SYMBOL | --constant-map SYMBOLS_CSV | --fixed-math OP ROWS | --trigon OP ROWS | --trigon-aggregate OP ROWS | --vector-transform ROWS | --matrix-multiply ROWS | --matrix-invert ROWS | --layout RECORD | --type-probe SYMBOL | --function-probe SYMBOL | --abi-value-echo TYPE ROWS | --compile-alias-probe MACRO TYPEDEF SIGNATURE | --macro-eval CASE_ID | --face-macro SRC_KIND SRC FACE_INDEX MACRO | --face-macro-flags MACRO FACE_INDEX FLAGS_CSV | --library-version PRESENT ROW_MASKS SENTINELS | --face-flags SRC_KIND SRC FACE_INDEX FLAG | --manager-reset-null | --manager-reset SRC_KIND SRC FACE_INDEX PX PY GID | --outline-render MODE CASE_ID | --outline-get-orientation CASE_ID | --new-memory-face SRC_KIND SRC FACE_INDEX PX PY | --new-memory-face-variants SRC_KIND SRC ROWS | --set-pixel-sizes SRC_KIND SRC FACE_INDEX PX PY | --set-char-size SRC_KIND SRC FACE_INDEX WIDTH HEIGHT HR VR | --set-char-sizes SRC_KIND SRC FACE_INDEX ROWS | --request-size SRC_KIND SRC FACE_INDEX ROWS | --select-size SRC_KIND SRC FACE_INDEX PX PY STRIKE | --select-size-null STRIKE | --new-size-null-face | --new-size-null-output SRC_KIND SRC FACE_INDEX | --new-size-sequence SRC_KIND SRC FACE_INDEX | --done-size-null | --done-size-sequence SRC_KIND SRC FACE_INDEX [REMOVE_ACTIVE] | --activate-size-null | --activate-size-sequence SRC_KIND SRC FACE_INDEX | --activate-select-size-sequence SRC_KIND SRC FACE_INDEX STRIKE | --size-metrics SRC_KIND SRC FACE_INDEX PX PY | --get-char-index SRC_KIND SRC FACE_INDEX PX PY CHAR | --face-get-char-variant-index SRC_KIND SRC FACE_INDEX PX PY CHAR SELECTOR | --face-get-char-variant-index-null CHAR SELECTOR | --get-kerning SRC_KIND SRC FACE_INDEX PX PY ROWS | --charmap-get-char-index SRC_KIND SRC FACE_INDEX PX PY PLATFORM ENCODING CHAR | --inspect-charmaps SRC_KIND SRC FACE_INDEX PX PY ENCODINGS CHARS | --set-charmap SRC_KIND SRC FACE_INDEX PX PY INDICES CHARS | --set-charmap-null-face | --set-charmap-variants SRC_KIND SRC FACE_INDEX FOREIGN_KIND FOREIGN_SRC FOREIGN_FACE_INDEX VARIANTS | --get-charmap-index SRC_KIND SRC FACE_INDEX PX PY | --get-charmap-index-variants SRC_KIND SRC FACE_INDEX VARIANTS | --get-cmap-format SRC_KIND SRC FACE_INDEX PX PY VARIANTS | --get-cmap-language-id SRC_KIND SRC FACE_INDEX PX PY VARIANTS | --select-charmap SRC_KIND SRC FACE_INDEX ENCODING CHARS | --set-lcd-filter LIBRARY_PRESENT FILTERS | --set-lcd-filter-weights LIBRARY_PRESENT WEIGHTS | --set-lcd-geometry LIBRARY_PRESENT GEOMETRY | --get-truetype-engine-type LIBRARY_PRESENT | --done-freetype MODE [SRC_KIND SRC FACE_INDEX] | --done-face MODE [SRC_KIND SRC FACE_INDEX] | --face-check-tt-patents MODE [SRC_KIND SRC FACE_INDEX] | --face-set-unpatented-hinting MODE VALUES [SRC_KIND SRC FACE_INDEX] | --outline-get-cbox-null-inputs SENTINEL | --get-fstype-flags SRC_KIND SRC FACE_INDEX PX PY [SYMBOL] | --get-postscript-name SRC_KIND SRC FACE_INDEX PX PY | --get-postscript-name-variants SRC_KIND SRC FACE_INDEX VARIANTS | --set-named-instance-null-face INSTANCE | --set-named-instance SRC_KIND SRC FACE_INDEX PRIOR_INSTANCE INSTANCE | --get-sfnt-name-count SRC_KIND SRC FACE_INDEX PX PY | --get-sfnt-name SRC_KIND SRC FACE_INDEX PX PY INDEXES | --get-sfnt-lang-tag SRC_KIND SRC FACE_INDEX PX PY LANG OUTPUT | --get-sfnt-lang-tag-variants SRC_KIND SRC FACE_INDEX PX PY ROWS | --get-sfnt-name-match SRC_KIND SRC FACE_INDEX PX PY PLATFORM ENCODING LANGUAGE NAME_ID | --sfnt-mac-encoding-record SRC_KIND SRC FACE_INDEX PX PY PLATFORM ENCODING LANGUAGE NAME_ID CODEPOINTS | --get-sfnt-os2-unicode-ranges SRC_KIND SRC FACE_INDEX PX PY CODEPOINTS | --get-first-char-null-face | --get-next-char-null-face STARTS_CSV | --get-first-char SRC_KIND SRC FACE_INDEX PX PY | --get-next-char-sequence SRC_KIND SRC FACE_INDEX PX PY MAX_STEPS | --get-next-char-starts SRC_KIND SRC FACE_INDEX PX PY STARTS_CSV | --load-char SRC_KIND SRC FACE_INDEX PX PY CHAR FLAGS | --load-glyph SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --load-glyph-from-char SRC_KIND SRC FACE_INDEX PX PY CHAR FLAGS | --load-glyph-num-glyphs SRC_KIND SRC FACE_INDEX PX PY FLAGS | --load-glyph-outline SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --outline-get-bbox SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --outline-get-cbox SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --glyph-get-cbox SRC_KIND SRC FACE_INDEX PX PY GID FLAGS MODES | --glyph-to-bitmap SRC_KIND SRC FACE_INDEX PX PY GID FLAGS MODE DESTROY | --glyphslot-slant SRC_KIND SRC FACE_INDEX PX PY ROWS | --glyphslot-oblique SRC_KIND SRC FACE_INDEX PX PY ROWS | --glyphslot-null-noop FUNCTION FIRST SECOND | --glyph-record SRC_KIND SRC FACE_INDEX PX PY GID FLAGS ACTION | --sbit-cache-lookup SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --inspect-glyph-metrics SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --inspect-glyph-slot SRC_KIND SRC FACE_INDEX PX PY GID FLAGS [MODE] | --get-advance SRC_KIND SRC FACE_INDEX PX PY GID FLAGS | --get-advances SRC_KIND SRC FACE_INDEX PX PY START COUNT FLAGS | --get-subglyph-info-null-slot SUB_INDEX | --get-subglyph-info-null-outputs SRC_KIND SRC FACE_INDEX PX PY GID FLAGS SUB_INDEX NULL_OUTPUTS | --get-subglyph-info SRC_KIND SRC FACE_INDEX PX PY GID FLAGS SUB_INDICES INVALID_SUB_INDICES | --render-glyph SRC_KIND SRC FACE_INDEX PX PY CHAR FLAGS MODE [REPEAT] | --render-glyph-index SRC_KIND SRC FACE_INDEX PX PY GID FLAGS MODE [REPEAT]\n");
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
