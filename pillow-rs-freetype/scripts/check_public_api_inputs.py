#!/usr/bin/env python3
"""Check public API manifest/input coverage and thin FFI ABI surfaces."""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests" / "manifest.yaml"
FIXTURE_DIR = ROOT / "tests" / "fixtures"
INPUT_DIR = ROOT / "tests" / "fixtures" / "inputs" / "public-api"
DEFAULT_AUDIT_JSON = ROOT / "target" / "api-abi-audit" / "api_abi_audit.json"
DEFAULT_ROUTE_AUDIT_JSON = ROOT / "target" / "api-abi-audit" / "route_audit.json"
DEFAULT_ROUTE_AUDIT_MD = ROOT / "target" / "api-abi-audit" / "route_audit.md"

FTERRDEF_EXACT_ERROR_BATCH = {
    ("load_glyph", "fterrdef.FT_Err_Bad_Argument.bytecode_invalid_jump_returns_error"),
    ("load_glyph", "fterrdef.FT_Err_Code_Overflow.bytecode_jump_past_range_returns_error"),
    ("load_glyph", "fterrdef.FT_Err_Code_Overflow.push_instruction_truncation_returns_error"),
    (
        "load_glyph",
        "fterrdef.FT_Err_Corrupted_Font_Header.autohint_zero_units_per_em_returns_error",
    ),
    (
        "load_glyph",
        "fterrdef.FT_Err_Could_Not_Find_Context.truetype_context_allocation_failure_returns_error",
    ),
    ("load_glyph", "fterrdef.FT_Err_DEF_In_Glyf_Bytecode.glyph_program_fdef_returns_error"),
    ("load_glyph", "fterrdef.FT_Err_Debug_OpCode.debug_opcode_returns_error"),
    ("load_glyph", "fterrdef.FT_Err_Divide_By_Zero.bytecode_div_zero_returns_error"),
    ("load_glyph", "fterrdef.FT_Err_ENDF_In_Exec_Stream.stray_endf_returns_error"),
    (
        "load_glyph",
        "fterrdef.FT_Err_Execution_Too_Long.opcode_counter_limit_returns_error",
    ),
    (
        "load_glyph",
        "fterrdef.FT_Err_Execution_Too_Long.negative_jump_limit_returns_error",
    ),
    ("load_glyph", "fterrdef.FT_Err_Glyph_Too_Big.ps_builder_large_outline_returns_error"),
    ("load_glyph", "fterrdef.FT_Err_Invalid_Opcode.tt_bytecode_invalid_opcode"),
    ("load_glyph", "fterrdef.FT_Err_Invalid_Reference.tt_bytecode_invalid_point_reference"),
    ("load_glyph", "fterrdef.FT_Err_Nested_DEFS.truetype_nested_fdef"),
    ("load_glyph", "fterrdef.FT_Err_Nested_DEFS.truetype_nested_idef"),
    ("load_glyph", "fterrdef.FT_Err_Stack_Overflow.tt_interpreter_stack_overflow"),
    ("load_glyph", "fterrdef.FT_Err_Stack_Overflow.cff_charstring_stack_overflow"),
    ("load_glyph", "fterrdef.FT_Err_Stack_Underflow.cff_charstring_missing_operands"),
    ("load_glyph", "fterrdef.FT_Err_Syntax_Error.charstring_or_afm_syntax_error"),
    ("load_glyph", "fterrdef.FT_Err_Too_Few_Arguments.tt_interpreter_argument_underflow"),
    ("load_glyph", "fterrdef.FT_Err_Too_Few_Arguments.cff_decoder_underflow_translation"),
    ("load_glyph", "fterrdef.FT_Err_Too_Many_Function_Defs.tt_fdef_limit_exceeded"),
    ("load_glyph", "fterrdef.FT_Err_Too_Many_Hints.tt_glyph_hint_limit"),
    (
        "load_glyph",
        "fterrdef.FT_Err_Too_Many_Instruction_Defs.tt_idef_limit_exceeded",
    ),
    ("load_glyph", "fterrdef.FT_Err_Unimplemented_Feature.unsupported_font_feature"),
}

FTMM_SUCCESS_OPERATIONS = {
    "ftmm.get_and_done_mm_var",
    "ftmm.get_mm_var",
    "ftmm.get_multi_master",
    "ftmm.get_var_axis_flags",
    "ftmm.get_var_blend_coordinates",
    "ftmm.get_var_design_coordinates",
    "ftmm.set_then_get_mm_blend_coordinates",
    "ftmm.set_mm_blend_coordinates",
    "ftmm.set_mm_design_coordinates",
    "ftmm.set_mm_weight_vector",
    "ftmm.set_var_blend_coordinates",
    "ftmm.set_var_design_coordinates",
}

FTDRIVER_SUCCESS_OPERATIONS = {
    "ftdriver.interpreter_version_glyph_output",
    "ftdriver.interpreter_version_property",
}

FTMODAPI_SUCCESS_OPERATIONS = {
    "freetype.module_interface_probe",
    "ftmodapi.inspect_module_flags",
}

SFNT_METADATA_SUCCESS_OPERATIONS = {
    "face.enumerate_charmaps",
    "freetype.enumerate_charmaps",
    "sfnt.charmap_and_name_metadata",
    "sfnt.enumerate_charmaps",
    "sfnt.enumerate_charmaps_and_names",
}

FTRENDER_SUCCESS_OPERATIONS = {
    "ftrender.set_renderer_then_render",
}

FTLOGGING_SUCCESS_OPERATIONS = {
    "ftlogging.set_default_log_handler",
    "ftlogging.set_log_handler",
    "ftlogging.set_log_handler_then_default",
    "ftlogging.trace_set_default_level",
    "ftlogging.trace_set_level",
}

API_SURFACE_EXCLUSIONS = {
    # Public header setup hooks, not user-callable API subjects.
    "fterrors.FT_NOERRORDEF_",
    "ftmac.FT_DEPRECATED_ATTRIBUTE",
    "ftmoderr.FT_MODERRDEF",
}

C_ABI_SOURCE = ROOT / "ffi-c" / "src" / "lib.rs"
C_ABI_HEADER = ROOT / "ffi-c" / "include" / "fontdone_ffi.h"
WASM_ABI_SOURCE = ROOT / "ffi-wasm" / "src" / "lib.rs"

WASM_EXPORTS = {
    "fontdone_wasm_malloc",
    "fontdone_wasm_free",
    "fontdone_wasm_gzip_uncompress",
    "fontdone_wasm_node_unref",
    "fontdone_wasm_stream_open_bzip2",
    "fontdone_wasm_stream_open_gzip",
    "fontdone_wasm_open_face",
    "fontdone_wasm_open_external_stream_face",
    "fontdone_wasm_open_face_with_name_options",
    "fontdone_wasm_done_face",
    "fontdone_wasm_new_size",
    "fontdone_wasm_new_size_out",
    "fontdone_wasm_activate_size",
    "fontdone_wasm_done_size",
    "fontdone_wasm_active_size",
    "fontdone_wasm_done_freetype",
    "fontdone_wasm_bitmap_blend",
    "fontdone_wasm_bitmap_convert",
    "fontdone_wasm_bitmap_copy",
    "fontdone_wasm_bitmap_done",
    "fontdone_wasm_bitmap_init",
    "fontdone_wasm_bitmap_new",
    "fontdone_wasm_list_add",
    "fontdone_wasm_list_insert",
    "fontdone_wasm_list_find",
    "fontdone_wasm_list_iterate",
    "fontdone_wasm_list_finalize",
    "fontdone_wasm_list_remove",
    "fontdone_wasm_list_up",
    "fontdone_wasm_glyphslot_own_bitmap",
    "fontdone_wasm_face_check_truetype_patents",
    "fontdone_wasm_face_set_unpatented_hinting",
    "fontdone_wasm_outline_get_cbox",
    "fontdone_wasm_glyph_get_cbox",
    "fontdone_wasm_get_glyph",
    "fontdone_wasm_get_glyph_from_face",
    "fontdone_wasm_glyph_copy",
    "fontdone_wasm_done_glyph",
    "fontdone_wasm_done_glyph_handle",
    "fontdone_wasm_glyph_transform",
    "fontdone_wasm_glyph_to_bitmap",
    "fontdone_wasm_glyph_to_bitmap_handle",
    "fontdone_wasm_outline_get_bbox",
    "fontdone_wasm_outline_get_bitmap",
    "fontdone_wasm_outline_render",
    "fontdone_wasm_outline_check",
    "fontdone_wasm_outline_copy",
    "fontdone_wasm_outline_embolden",
    "fontdone_wasm_outline_embolden_xy",
    "fontdone_wasm_outline_get_inside_border",
    "fontdone_wasm_outline_new",
    "fontdone_wasm_outline_done",
    "fontdone_wasm_outline_get_orientation",
    "fontdone_wasm_outline_get_outside_border",
    "fontdone_wasm_outline_reverse",
    "fontdone_wasm_outline_transform",
    "fontdone_wasm_outline_translate",
    "fontdone_wasm_get_truetype_engine_type",
    "fontdone_wasm_face_properties_one",
    "fontdone_wasm_property_get",
    "fontdone_wasm_property_glyph_to_script_map_invalid_face",
    "fontdone_wasm_property_increase_x_height_invalid_face",
    "fontdone_wasm_property_increase_x_height_set_then_get",
    "fontdone_wasm_property_set_then_get",
    "fontdone_wasm_library_set_lcd_filter",
    "fontdone_wasm_library_set_lcd_filter_weights",
    "fontdone_wasm_library_set_lcd_geometry",
    "fontdone_wasm_palette_data_get",
    "fontdone_wasm_palette_select",
    "fontdone_wasm_palette_set_foreground_color",
    "fontdone_wasm_get_color_glyph_layer",
    "fontdone_wasm_get_color_glyph_clipbox",
    "fontdone_wasm_get_color_glyph_paint",
    "fontdone_wasm_get_paint",
    "fontdone_wasm_get_paint_layers",
    "fontdone_wasm_get_colorline_stops",
    "fontdone_wasm_mul_div",
    "fontdone_wasm_mul_fix",
    "fontdone_wasm_div_fix",
    "fontdone_wasm_error_string",
    "fontdone_wasm_get_ps_font_info",
    "fontdone_wasm_get_ps_font_private",
    "fontdone_wasm_has_ps_glyph_names",
    "fontdone_wasm_get_ps_font_value",
    "fontdone_wasm_open_type_validate",
    "fontdone_wasm_open_type_free",
    "fontdone_wasm_truetype_gx_free",
    "fontdone_wasm_classic_kern_free",
    "fontdone_wasm_classic_kern_validate",
    "fontdone_wasm_round_fix",
    "fontdone_wasm_ceil_fix",
    "fontdone_wasm_floor_fix",
    "fontdone_wasm_sin",
    "fontdone_wasm_cos",
    "fontdone_wasm_tan",
    "fontdone_wasm_atan2",
    "fontdone_wasm_angle_diff",
    "fontdone_wasm_vector_unit",
    "fontdone_wasm_vector_rotate",
    "fontdone_wasm_vector_length",
    "fontdone_wasm_vector_polarize",
    "fontdone_wasm_vector_from_polar",
    "fontdone_wasm_vector_transform",
    "fontdone_wasm_matrix_multiply",
    "fontdone_wasm_matrix_invert",
    "fontdone_wasm_set_pixel_sizes",
    "fontdone_wasm_set_transform",
    "fontdone_wasm_set_char_size",
    "fontdone_wasm_request_size",
    "fontdone_wasm_select_size",
    "fontdone_wasm_get_char_index",
    "fontdone_wasm_get_char_variant_index",
    "fontdone_wasm_get_char_variant_is_default",
    "fontdone_wasm_get_variant_selectors",
    "fontdone_wasm_get_variants_of_char",
    "fontdone_wasm_get_chars_of_variant",
    "fontdone_wasm_get_kerning",
    "fontdone_wasm_get_pfr_kerning",
    "fontdone_wasm_select_charmap",
    "fontdone_wasm_get_charmap_count",
    "fontdone_wasm_get_active_charmap_index",
    "fontdone_wasm_get_charmap",
    "fontdone_wasm_get_cmap_format",
    "fontdone_wasm_get_cmap_language_id",
    "fontdone_wasm_set_charmap",
    "fontdone_wasm_set_charmap_from_face",
    "fontdone_wasm_get_fstype_flags",
    "fontdone_wasm_attach_stream",
    "fontdone_wasm_get_track_kerning",
    "fontdone_wasm_get_gasp",
    "fontdone_wasm_get_glyph_name",
    "fontdone_wasm_get_name_index",
    "fontdone_wasm_get_postscript_name",
    "fontdone_wasm_get_font_format",
    "fontdone_wasm_get_x11_font_format",
    "fontdone_wasm_set_named_instance",
    "fontdone_wasm_get_mm_blend_coordinates",
    "fontdone_wasm_get_mm_var",
    "fontdone_wasm_get_multi_master",
    "fontdone_wasm_get_mm_weight_vector",
    "fontdone_wasm_get_var_blend_coordinates",
    "fontdone_wasm_get_var_axis_flags",
    "fontdone_wasm_get_var_design_coordinates",
    "fontdone_wasm_set_mm_blend_coordinates",
    "fontdone_wasm_set_mm_design_coordinates",
    "fontdone_wasm_set_mm_weight_vector",
    "fontdone_wasm_set_var_blend_coordinates",
    "fontdone_wasm_set_var_design_coordinates",
    "fontdone_wasm_get_default_named_instance",
    "fontdone_wasm_get_winfnt_header",
    "fontdone_wasm_get_bdf_property",
    "fontdone_wasm_get_bdf_charset_id",
    "fontdone_wasm_get_cid_is_internally_cid_keyed",
    "fontdone_wasm_get_cid_from_glyph_index",
    "fontdone_wasm_get_cid_registry_ordering_supplement",
    "fontdone_wasm_get_sfnt_name_count",
    "fontdone_wasm_get_sfnt_name",
    "fontdone_wasm_get_sfnt_os2",
    "fontdone_wasm_get_sfnt_vhea",
    "fontdone_wasm_get_sfnt_maxp",
    "fontdone_wasm_load_sfnt_table",
    "fontdone_wasm_sfnt_table_info",
    "fontdone_wasm_get_first_char",
    "fontdone_wasm_get_next_char",
    "fontdone_wasm_library_version",
    "fontdone_wasm_load_char",
    "fontdone_wasm_load_glyph",
    "fontdone_wasm_get_advance",
    "fontdone_wasm_get_advances",
    "fontdone_wasm_get_subglyph_info",
    "fontdone_wasm_render_glyph",
    "fontdone_wasm_bitmap_embolden",
    "fontdone_wasm_glyphslot_adjust_weight",
    "fontdone_wasm_glyphslot_embolden",
    "fontdone_wasm_glyphslot_oblique",
    "fontdone_wasm_glyphslot_slant",
    "fontdone_wasm_get_slot",
    "fontdone_wasm_size_metrics",
}

DISALLOWED_FFI_SNIPPETS = (
    "std::fs",
    "std::path",
    "std::process",
    "serde",
    "serde_json",
    "Command::",
    "File::",
    "read_to_",
    "read(",
    "write(",
    "fontdone_test",
)

COMPILE_CONTRACT_OPERATIONS = {
    "constant",
    "constant_map",
    "record_layout",
    "abi_type_probe",
    "abi_type_map_probe",
    "abi_function_probe",
    "abi.compile_alias_probe",
    "abi.value_echo",
    "macro_eval",
    "macro_compile_probe",
    "face_macro_flags",
    "freetype.vector_transform",
    "ftlzw.stream_open_lzw_abi",
    "ftlist.list_insert_abi",
    "ftlist.list_iterate_abi",
    "ftlist.list_remove_abi",
    "ftlist.list_up_abi",
    "ftlogging.set_default_log_handler_abi",
    "ftlogging.set_log_handler_abi",
    "ftlogging.trace_set_default_level_abi",
    "ftlogging.trace_set_level_abi",
    "ftmm.done_mm_var_abi",
    "ftmm.get_default_named_instance_abi",
    "ftmm.get_mm_blend_coordinates_abi",
    "ftmm.get_mm_var_abi",
    "ftmm.get_mm_weightvector_abi",
    "ftmm.get_multi_master_abi",
    "ftmm.get_var_axis_flags_abi",
    "ftstroke.stroker_type_abi",
    "ftwinfnt.get_winfnt_header_abi",
    "ftwinfnt.winfnt_header_rec_field_order",
    "ftwinfnt.winfnt_header_type_import",
    "ftwinfnt.winfnt_header_type_abi",
    "ftwinfnt.winfnt_header_rec_abi",
    "ftglyph.matrix_multiply",
    "ftglyph.matrix_invert",
    "otsvg.svg_document_type_import",
    "otsvg.svg_document_type_abi",
    "otsvg.svg_document_rec_abi",
    "t1tables.t1_blend_flags_enum",
    "t1tables.t1_blend_flags_sentinel",
    "t1tables.t1_encoding_type_enum",
}

REAL_PARITY_OPERATIONS = {
    "new_memory_face",
    "set_pixel_sizes",
    "set_char_size",
    "freetype.request_size",
    "freetype.select_size",
    "size_metrics",
    "get_char_index",
    "charmap.get_char_index",
    "freetype.select_charmap",
    "freetype.set_charmap",
    "freetype.inspect_face_rec",
    "freetype.inspect_available_sizes",
    "freetype.inspect_charmaps",
    "freetype.charmap_ownership",
    "freetype.get_charmap_index",
    "freetype.face_flags",
    "freetype.face_properties",
    "freetype.get_fstype_flags",
    "freetype.get_kerning",
    "freetype.get_track_kerning",
    "freetype.attach_file",
    "freetype.attach_stream",
    "freetype.ceil_fix",
    "freetype.floor_fix",
    "freetype.round_fix",
    "freetype.mul_div",
    "freetype.mul_fix",
    "freetype.div_fix",
    "freetype.get_postscript_name",
    "freetype.face_get_char_variant_index",
    "freetype.face_get_char_variant_is_default",
    "freetype.face_get_variant_selectors",
    "freetype.face_get_variants_of_char",
    "freetype.face_get_chars_of_variant",
    "ftmm.set_named_instance",
    "ftmm.get_default_named_instance",
    "winfnt.get_header",
    "ftwinfnt.get_winfnt_header",
    "freetype.get_glyph_name",
    "freetype.get_name_index",
    "freetype.new_face",
    "freetype.set_transform",
    "freetype.get_transform",
    "freetype.reference_face",
    "ftsnames.get_sfnt_name_count",
    "ftsnames.get_sfnt_name",
    "ftsnames.get_sfnt_name_by_record",
    "ftsnames.get_sfnt_name_group",
    "ftsnames.get_sfnt_lang_tag",
    "sfnt.get_name",
    "sfnt.get_sfnt_name",
    "sfnt.get_os2_unicode_ranges",
    "sfnt.get_sfnt_table",
    "sfnt.get_sfnt_table.record",
    "sfnt.get_sfnt_table.head",
    "sfnt.get_sfnt_table.maxp",
    "sfnt.get_sfnt_table.hhea",
    "sfnt.get_sfnt_table.hhea.after_variation",
    "face.load_then_get_sfnt_table.maxp",
    "sfnt.load_sfnt_table",
    "sfnt.table_info",
    "sfnt.mac_encoding_record",
    "fterrors.error_string",
    "freetype.get_first_char",
    "freetype.get_next_char",
    "freetype.init_free_type",
    "freetype.library_version",
    "ftmodapi.get_truetype_engine_type",
    "freetype.done_freetype",
    "freetype.done_face",
    "freetype.face_check_truetype_patents",
    "freetype.face_set_unpatented_hinting",
    "ftlcdfil.set_lcd_filter",
    "ftlcdfil.set_lcd_filter_weights",
    "ftlcdfil.set_lcd_geometry",
    "load_char",
    "load_glyph",
    "freetype.inspect_glyph_metrics",
    "freetype.inspect_glyph_slot",
    "freetype.get_subglyph_info",
    "ftpfr.get_pfr_kerning",
    "freetype.load_glyph_outline",
    "ftbbox.outline_get_bbox",
    "ftimage.outline_decompose",
    "ftoutln.outline_decompose",
    "ftoutln.outline_get_cbox",
    "ftoutln.outline_get_orientation",
    "ftoutln.get_orientation",
    "ftoutln.get_orientation_after_mutation",
    "ftoutln.outline_check",
    "ftoutln.outline_copy",
    "ftoutln.outline_embolden",
    "ftoutln.outline_embolden_xy",
    "ftoutln.outline_done",
    "ftoutln.outline_new",
    "ftoutln.outline_reverse",
    "ftoutln.outline_transform",
    "ftoutln.outline_translate",
    "ftstroke.outline_border_orientation_pair",
    "ftstroke.outline_get_inside_border",
    "ftstroke.outline_get_outside_border",
    "ftglyph.glyph_get_cbox",
    "ftglyph.glyph_to_bitmap",
    "ftglyph.get_glyph",
    "ftglyph.glyph_copy",
    "ftglyph.record_inspect",
    "ftglyph.type_runtime",
    "ftbitmap.bitmap_blend",
    "ftbitmap.bitmap_convert",
    "ftbitmap.bitmap_copy",
    "ftbitmap.bitmap_done",
    "ftbitmap.bitmap_embolden",
    "ftbitmap.glyphslot_own_bitmap",
    "ftbitmap.bitmap_init",
    "ftbitmap.bitmap_new",
    "ftlist.list_add",
    "ftlist.list_insert",
    "ftlist.list_find",
    "ftlist.list_finalize",
    "ftlist.list_remove",
    "ftlist.list_up",
    "ftcache.sbit_cache_lookup",
    "ftcache.manager_reset",
    "ftoutln.outline_render",
    "ftoutln.outline_render_direct",
    "ftadvanc.get_advance",
    "ftadvanc.get_advances",
    "render_glyph",
    "ftsynth.glyphslot_slant_after_load",
    "ftsynth.glyphslot_oblique_after_load",
    "ftsynth.glyphslot_adjust_weight_after_load",
    "ftsynth.glyphslot_embolden_after_load",
    "ftgasp.get_gasp",
    "tttables.get_cmap_format",
    "tttables.get_cmap_language_id",
    "ftsizes.new_size_sequence",
    "ftsizes.done_size_sequence",
    "ftsizes.activate_size_sequence",
    "ftsizes.activate_select_size_sequence",
}

EXPLICIT_UNSUPPORTED_OPERATIONS = set()

AUDIT_ONLY_PENDING_CORE_CASES = {
    "tttables.TT_VertHeader.sfnt_table_present_runtime.mvar_variation",
}

SHARED_RUNTIME_FALLBACK_CASES = {
    # The unified runtime currently compares one shared Rust direct-span
    # fallback for all three actual lanes, not exported C/WASM public routes.
    "ftimage.FT_Raster_Span_Func.direct_render_emits_spans",
}

PLACEHOLDER_STYLE_CATEGORIES = {
    "generic-fallback",
    "generic-error-fallback",
    "null-error-fallback",
    "explicit-unsupported",
    "void-fallback",
}

SUPPLEMENTARY_SAFE_API_FLAGS = {
    "assert_font_render_mode_agrees": "Font render helpers",
    "assert_font_truetype_constructor_agrees": "Font constructors",
    "assert_font_face_count_agrees": "Font face-count helper",
    "assert_font_empty_text_render": "Font render helpers",
    "assert_font_getmetrics_agrees": "Font convenience metrics",
    "assert_font_getlength_agrees": "Font convenience metrics",
    "assert_font_glyph_metrics_agrees": "Font convenience metrics",
    "assert_font_getmask_agrees": "Font convenience masks",
    "assert_font_empty_text_mask": "Font convenience masks",
    "assert_font_getkerning_agrees": "Font convenience kerning",
    "assert_font_hori_advance_agrees": "Font convenience advances",
    "assert_font_charmap_accessors_agree": "Font charmap convenience accessors",
}

NON_FREETYPE_GOAL_SURFACES = (
    "Pillow adapter methods in imagingft.rs model Pillow _imagingft.c rather "
    "than a public C FreeType API.",
    "High-level fontdone::Font convenience methods are supplementary unless "
    "routed through a public FreeType manifest row.",
    "Former local fontdone::ffi::handles inspection helpers were removed from "
    "the Rust facade; face metrics, face flags, active size, and charmaps now "
    "use public FT_Face-shaped fields, and SFNT/CMAP metadata routes through "
    "public FreeType-shaped functions.",
)

COMPILE_CONTRACT_PREFIXES = (
    "abi.",
    "c_compile.",
    "c_preprocessor.",
    "cxx_preprocessor.",
    "fttypes.",
)

REAL_PARITY_PREFIXES = (
    "freetype.face_macro",
    "fttrigon.",
)


@dataclass(frozen=True)
class ManifestSubject:
    subject_id: str
    kind: str
    symbol: str
    header: str
    cases: set[str]


@dataclass(frozen=True)
class ConcreteInput:
    subject: str
    case_id: str
    case: str
    operation: str
    variant_id: str | None
    expect_error: bool
    compare_error_output: bool
    allow_oracle_errors: bool
    expectation_status: str
    assets: dict[str, object]
    params: dict[str, object]


CASE_COLLISION_CARRIERS = {
    # FreeType still exposes these deprecated mixed-case aliases alongside the
    # modern macros.  Store each alias with its modern spelling so a checkout
    # remains representable on case-insensitive filesystems.
    "ftimage.FT_Curve_Tag_Conic": "ftimage.FT_CURVE_TAG_CONIC",
    "ftimage.FT_Curve_Tag_Cubic": "ftimage.FT_CURVE_TAG_CUBIC",
    "ftimage.FT_Curve_Tag_On": "ftimage.FT_CURVE_TAG_ON",
    "ftimage.FT_Curve_Tag_Touch_X": "ftimage.FT_CURVE_TAG_TOUCH_X",
    "ftimage.FT_Curve_Tag_Touch_Y": "ftimage.FT_CURVE_TAG_TOUCH_Y",
    # FT_Bool is the public typedef; FT_BOOL is the distinct conversion macro.
    "fttypes.FT_BOOL": "fttypes.FT_Bool",
}


def filename_for_subject(subject: str) -> str:
    carrier = CASE_COLLISION_CARRIERS.get(subject, subject)
    return re.sub(r"[^A-Za-z0-9_.-]", "_", carrier) + ".json"


def read_manifest() -> dict[str, ManifestSubject]:
    subjects: dict[str, dict[str, object]] = {}
    current: str | None = None
    in_cases = False
    for line in MANIFEST.read_text().splitlines():
        if line.startswith("  - id: "):
            current = line.split(": ", 1)[1].strip()
            if current in subjects:
                raise RuntimeError(f"duplicate manifest subject id {current}")
            subjects[current] = {
                "kind": "",
                "symbol": "",
                "header": "",
                "cases": set(),
            }
            in_cases = False
        elif line.startswith("    kind: "):
            require_current(current)
            subjects[current]["kind"] = line.split(": ", 1)[1].strip()
        elif line.startswith("    symbol: "):
            require_current(current)
            subjects[current]["symbol"] = line.split(": ", 1)[1].strip()
        elif line.startswith("    header: "):
            require_current(current)
            subjects[current]["header"] = line.split(": ", 1)[1].strip()
        elif line.startswith("    cases:"):
            require_current(current)
            in_cases = True
        elif line.startswith("      - id: "):
            require_current(current)
            if not in_cases:
                raise RuntimeError(f"case outside cases block for {current}")
            case_id = line.split(": ", 1)[1].strip()
            cases = subjects[current]["cases"]
            assert isinstance(cases, set)
            if case_id in cases:
                raise RuntimeError(f"{current}: duplicate manifest case {case_id}")
            cases.add(case_id)

    result: dict[str, ManifestSubject] = {}
    seen_symbols: dict[tuple[str, str], str] = {}
    for subject_id, raw in subjects.items():
        cases = raw["cases"]
        assert isinstance(cases, set)
        subject = ManifestSubject(
            subject_id=subject_id,
            kind=str(raw["kind"]),
            symbol=str(raw["symbol"]),
            header=str(raw["header"]),
            cases=cases,
        )
        if not subject.kind:
            raise RuntimeError(f"{subject_id}: missing kind")
        if not subject.symbol:
            raise RuntimeError(f"{subject_id}: missing symbol")
        if not subject.header:
            raise RuntimeError(f"{subject_id}: missing header")
        if not subject.cases:
            raise RuntimeError(f"{subject_id}: missing cases")
        key = (subject.header, subject.symbol)
        if key in seen_symbols:
            raise RuntimeError(
                f"{subject_id}: duplicates symbol {subject.symbol} in {seen_symbols[key]}"
            )
        seen_symbols[key] = subject_id
        result[subject_id] = subject
    return result


def require_current(current: str | None) -> None:
    if current is None:
        raise RuntimeError("manifest field before subject")


def check_explicit_inputs(subject_id: str, index: int, inputs: object) -> list[str]:
    prefix = f"{subject_id}: cases[{index}]"
    if not isinstance(inputs, dict):
        return [f"{prefix} inputs must be object"]
    errors: list[str] = []
    if "variability" in inputs:
        errors.append(f"{prefix} uses forbidden implicit variability")
    assets = inputs.get("assets", {})
    if isinstance(assets, dict) and "font_folder" in assets:
        errors.append(f"{prefix} uses forbidden runtime font-folder discovery")
    variants = inputs.get("variants", [])
    if not variants:
        return errors
    if not isinstance(variants, list):
        errors.append(f"{prefix} variants must be list")
        return errors
    if assets or inputs.get("params") not in (None, {}):
        errors.append(f"{prefix} mixes variants with direct assets or params")
    seen: set[str] = set()
    for variant_index, variant in enumerate(variants):
        variant_prefix = f"{prefix} variants[{variant_index}]"
        if not isinstance(variant, dict):
            errors.append(f"{variant_prefix} must be object")
            continue
        variant_id = variant.get("id")
        if not isinstance(variant_id, str) or not variant_id:
            errors.append(f"{variant_prefix} needs a non-empty id")
        elif variant_id in seen:
            errors.append(f"{variant_prefix} duplicates id {variant_id}")
        else:
            seen.add(variant_id)
        coverage = variant.get("coverage")
        if not isinstance(coverage, list) or not coverage or not all(
            isinstance(item, str) and item for item in coverage
        ):
            errors.append(f"{variant_prefix} needs non-empty coverage intent strings")
        variant_assets = variant.get("assets", {})
        if isinstance(variant_assets, dict) and "font_folder" in variant_assets:
            errors.append(f"{variant_prefix} uses forbidden runtime font-folder discovery")
    return errors


def check_file(subject: ManifestSubject) -> list[str]:
    errors: list[str] = []
    path = INPUT_DIR / filename_for_subject(subject.subject_id)
    if not path.exists():
        return [f"{subject.subject_id}: missing {path.relative_to(ROOT)}"]

    try:
        data = json.loads(path.read_text())
    except Exception as exc:  # noqa: BLE001 - report parse failure with path.
        return [f"{subject.subject_id}: invalid json: {exc}"]

    if data.get("version") != 1:
        errors.append(f"{subject.subject_id}: version must be 1")
    declared_subjects = data.get("subjects")
    if declared_subjects is None:
        declared_subjects = [data.get("subject")]
    if not isinstance(declared_subjects, list) or not all(
        isinstance(item, str) for item in declared_subjects
    ):
        errors.append(f"{subject.subject_id}: top-level subjects must be a string list")
        declared_subjects = []
    elif subject.subject_id not in declared_subjects:
        errors.append(f"{subject.subject_id}: top-level subject mismatch")
    manifest_cases_field = data.get("manifest_cases")
    if manifest_cases_field is not None:
        if not isinstance(manifest_cases_field, list):
            errors.append(f"{subject.subject_id}: manifest_cases must be list")
        else:
            unknown = sorted(set(manifest_cases_field) - subject.cases)
            if unknown:
                errors.append(f"{subject.subject_id}: manifest_cases contains unknown cases {unknown}")

    all_cases = data.get("cases")
    if not isinstance(all_cases, list) or not all_cases:
        errors.append(f"{subject.subject_id}: cases must be a non-empty list")
        return errors
    for index, case in enumerate(all_cases):
        if not isinstance(case, dict):
            errors.append(f"{subject.subject_id}: cases[{index}] must be object")
        elif case.get("subject") not in declared_subjects:
            errors.append(f"{subject.subject_id}: cases[{index}] has undeclared subject")
    cases = [
        case
        for case in all_cases
        if isinstance(case, dict) and case.get("subject") == subject.subject_id
    ]
    if not cases:
        errors.append(f"{subject.subject_id}: cases must cover the subject")
        return errors

    covered: set[str] = set()
    for index, case in enumerate(cases):
        manifest_case = case.get("case")
        covers_manifest_cases = case.get("covers_manifest_cases", [])
        if not isinstance(covers_manifest_cases, list):
            errors.append(f"{subject.subject_id}: cases[{index}] covers_manifest_cases must be list")
            covers_manifest_cases = []

        if not isinstance(manifest_case, str):
            errors.append(f"{subject.subject_id}: cases[{index}] missing case")
        elif manifest_case not in subject.cases:
            if not covers_manifest_cases:
                errors.append(
                    f"{subject.subject_id}: cases[{index}] unknown manifest case {manifest_case} "
                    "without covers_manifest_cases"
                )
        else:
            covered.add(manifest_case)
        for covered_case in covers_manifest_cases:
            if not isinstance(covered_case, str) or covered_case not in subject.cases:
                errors.append(
                    f"{subject.subject_id}: cases[{index}] invalid covered manifest case {covered_case}"
                )
            else:
                covered.add(covered_case)
        for key in ("case_id", "operation", "schema", "inputs", "expectation"):
            if key not in case:
                errors.append(f"{subject.subject_id}: cases[{index}] missing {key}")
        errors.extend(check_explicit_inputs(subject.subject_id, index, case.get("inputs")))
        if case.get("schema") == "scalar":
            errors.append(f"{subject.subject_id}: cases[{index}] uses legacy scalar schema")
        expectation = case.get("expectation")
        if isinstance(expectation, dict):
            if "output_shape" not in expectation:
                errors.append(f"{subject.subject_id}: cases[{index}] expectation missing output_shape")
            if "compare" not in expectation:
                errors.append(f"{subject.subject_id}: cases[{index}] expectation missing compare")
        else:
            errors.append(f"{subject.subject_id}: cases[{index}] expectation must be object")

    missing = sorted(subject.cases - covered)
    if missing:
        errors.append(f"{subject.subject_id}: missing manifest cases {missing}")
    return errors


def public_surface_subjects(audit_json: Path) -> dict[str, tuple[str, str]]:
    data = json.loads(audit_json.read_text())
    expected: dict[str, tuple[str, str]] = {}
    for bucket, key in (
        ("functions", "symbol"),
        ("types", "type"),
        ("constants", "constant"),
        ("enum_variants", "constant"),
        ("error_codes", "constant"),
    ):
        for row in data[bucket]:
            subject = subject_for_c_symbol(row["c_file"], row[key])
            if subject not in API_SURFACE_EXCLUSIONS:
                expected[subject] = (row["c_file"], row[key])
    return expected


def subject_for_c_symbol(c_file: str, symbol: str) -> str:
    return f"{Path(c_file).stem}.{symbol}"


def check_manifest_public_surface(
    subjects: dict[str, ManifestSubject],
    audit_json: Path,
) -> list[str]:
    errors: list[str] = []
    expected = public_surface_subjects(audit_json)
    manifest_ids = set(subjects)
    expected_ids = set(expected)
    for subject in sorted(expected_ids - manifest_ids):
        c_file, symbol = expected[subject]
        errors.append(f"manifest missing public C subject {subject} ({symbol} in {c_file})")
    for subject in sorted(manifest_ids - expected_ids):
        errors.append(f"manifest has non-public or duplicate-derived subject {subject}")
    for subject_id in sorted(manifest_ids & expected_ids):
        c_file, symbol = expected[subject_id]
        subject = subjects[subject_id]
        if subject.header != c_file:
            errors.append(f"{subject_id}: header mismatch manifest={subject.header} audit={c_file}")
        if subject.symbol != symbol:
            errors.append(f"{subject_id}: symbol mismatch manifest={subject.symbol} audit={symbol}")
    return errors


def check_input_directory(subjects: dict[str, ManifestSubject]) -> list[str]:
    errors: list[str] = []
    expected_paths = {
        INPUT_DIR / filename_for_subject(subject_id)
        for subject_id in subjects
    }
    actual_paths = set(INPUT_DIR.rglob("*.json"))
    for path in sorted(expected_paths - actual_paths):
        errors.append(f"missing public API input file {path.relative_to(ROOT)}")
    for path in sorted(actual_paths - expected_paths):
        errors.append(f"extra public API input file {path.relative_to(ROOT)}")
    return errors


def check_ffi_abi_surface(audit_json: Path) -> list[str]:
    errors: list[str] = []
    audit = json.loads(audit_json.read_text())
    public_functions = {row["symbol"] for row in audit["functions"]}

    c_exports = exported_functions(C_ABI_SOURCE)
    for export in sorted(c_exports):
        if export not in public_functions:
            errors.append(f"ffi-c exports non-FreeType C symbol {export}")
    header_text = C_ABI_HEADER.read_text()
    if "fontdone_" in header_text:
        errors.append("ffi-c public header exposes fontdone_* helper symbols")
    for export in sorted(c_exports):
        if not re.search(r"\b" + re.escape(export) + r"\s*\(", header_text):
            errors.append(f"ffi-c header missing exported symbol {export}")

    wasm_exports = exported_functions(WASM_ABI_SOURCE)
    extra_wasm = wasm_exports - WASM_EXPORTS
    missing_wasm = WASM_EXPORTS - wasm_exports
    for export in sorted(extra_wasm):
        errors.append(f"ffi-wasm exports unexpected symbol {export}")
    for export in sorted(missing_wasm):
        errors.append(f"ffi-wasm missing expected symbol {export}")

    for path in (C_ABI_SOURCE, WASM_ABI_SOURCE):
        text = path.read_text()
        for snippet in DISALLOWED_FFI_SNIPPETS:
            if snippet in text:
                errors.append(f"{path.relative_to(ROOT)} contains disallowed wrapper logic {snippet}")
    return errors


def exported_functions(path: Path) -> set[str]:
    text = path.read_text()
    pattern = r"#\[unsafe\(no_mangle\)\]\s*pub\s+extern\s+\"C\"\s+fn\s+([A-Za-z_][A-Za-z0-9_]*)"
    return set(re.findall(pattern, text))


def concrete_inputs(items: dict[str, ManifestSubject]) -> list[ConcreteInput]:
    rows: list[ConcreteInput] = []
    for subject_id in sorted(items):
        path = INPUT_DIR / filename_for_subject(subject_id)
        data = json.loads(path.read_text())
        for case in data.get("cases", []):
            if case.get("subject") != subject_id:
                continue
            expectation = object_dict(case.get("expectation", {}))
            compare = object_dict(expectation.get("compare", {}))
            expectation_status = str(expectation.get("status", ""))
            inputs = case.get("inputs", {})
            if not isinstance(inputs, dict):
                continue
            variants = inputs.get("variants")
            if isinstance(variants, list) and variants:
                for variant in variants:
                    if not isinstance(variant, dict):
                        continue
                    variant_expectation = object_dict(variant.get("expectation", {}))
                    variant_compare = object_dict(variant_expectation.get("compare", {}))
                    rows.append(
                        ConcreteInput(
                            subject=subject_id,
                            case_id=str(case.get("case_id", "")),
                            case=str(case.get("case", "")),
                            operation=str(case.get("operation", "")),
                            variant_id=str(variant.get("id", "")) or None,
                            expect_error=bool(variant.get("expect_error", case.get("expect_error", False))),
                            compare_error_output=bool(
                                variant_compare.get(
                                    "compare_error_output",
                                    compare.get("compare_error_output", False),
                                )
                            )
                            or exact_error_public_route(
                                str(case.get("operation", "")),
                                str(case.get("case_id", "")),
                                bool(variant.get("expect_error", case.get("expect_error", False))),
                            ),
                            allow_oracle_errors=bool(
                                variant_compare.get(
                                    "allow_oracle_errors",
                                    compare.get("allow_oracle_errors", False),
                                )
                            ),
                            expectation_status=expectation_status,
                            assets=object_dict(variant.get("assets", {})),
                            params=object_dict(variant.get("params", {})),
                        )
                    )
            else:
                rows.append(
                    ConcreteInput(
                        subject=subject_id,
                        case_id=str(case.get("case_id", "")),
                        case=str(case.get("case", "")),
                        operation=str(case.get("operation", "")),
                        variant_id=None,
                        expect_error=bool(case.get("expect_error", False)),
                        compare_error_output=bool(compare.get("compare_error_output", False))
                        or exact_error_public_route(
                            str(case.get("operation", "")),
                            str(case.get("case_id", "")),
                            bool(case.get("expect_error", False)),
                        ),
                        allow_oracle_errors=bool(compare.get("allow_oracle_errors", False)),
                        expectation_status=expectation_status,
                        assets=object_dict(inputs.get("assets", {})),
                        params=object_dict(inputs.get("params", {})),
                    )
                )
    return rows


def object_dict(value: object) -> dict[str, object]:
    return value if isinstance(value, dict) else {}


def exact_error_public_family(operation: str, expect_error: bool) -> bool:
    """Require exact status and output for public error routes with dedicated runners."""
    return expect_error and (
        operation.startswith("ftsizes.")
        or operation in {"freetype.get_kerning", "freetype.get_subglyph_info"}
    )


def exact_error_public_route(operation: str, case_id: str, expect_error: bool) -> bool:
    """Mirror harness-promoted exact-error routes in the route audit ledger."""
    if exact_error_public_family(operation, expect_error):
        return True
    return expect_error and case_id in {
        "ftmm.FT_Get_Var_Design_Coordinates.error_null_coords",
        "ftmm.FT_Get_Var_Blend_Coordinates.error_null_coords",
        "ftmm.FT_Get_Var_Blend_Coordinates.error_non_variable_or_invalid_face",
        "ftmm.FT_Get_MM_Blend_Coordinates.invalid_face_or_coords_error",
        "ftmm.FT_Get_MM_Var.null_output_error",
        "ftmm.FT_Get_MM_Var.invalid_or_non_variable_face_error",
        "ftmm.FT_Get_MM_WeightVector.len_without_buffer_error",
        "ftmm.FT_Get_MM_WeightVector.unsupported_face_error",
        "ftmm.FT_Get_Multi_Master.true_type_or_opentype_variation_error",
        "ftmm.FT_Get_Multi_Master.invalid_or_non_variable_face_error",
        "ftmm.FT_Set_MM_Design_Coordinates.error_non_adobe_variation_face",
        "ftmm.FT_Set_MM_WeightVector.error_null_weightvector_with_nonzero_len",
        "ftmm.FT_Set_MM_WeightVector.error_unsupported_on_true_type_variations",
        "ftmm.FT_Set_Var_Design_Coordinates.error_null_coords_with_nonzero_count",
        "fterrdef.FT_Err_Array_Too_Large.ttc_header_overflow_returns_error",
        "fterrdef.FT_Err_Bbx_Too_Big.bdf_bitmap_size_overflow_returns_error",
        "fterrdef.FT_Err_Corrupted_Font_Glyphs.bdf_glyph_parse_failure_returns_error",
        "fterrdef.FT_Err_Corrupted_Font_Header.bdf_header_parse_failure_returns_error",
        "fterrdef.FT_Err_Missing_Bbx_Field.bdf_bitmap_before_bbx",
        "fterrdef.FT_Err_Missing_Encoding_Field.bdf_glyph_without_encoding",
        "fterrdef.FT_Err_Missing_Font_Field.bdf_chars_before_font",
        "fterrdef.FT_Err_Missing_Fontboundingbox_Field.bdf_chars_before_fontboundingbox",
        "fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error",
        "fterrdef.FT_Err_Invalid_Post_Table_Format.sfnt_post_format_rejected",
        "fterrdef.FT_Err_Missing_Size_Field.bdf_chars_before_size",
        "fterrdef.FT_Err_Missing_Startchar_Field.bdf_encoding_before_startchar",
        "fterrdef.FT_Err_Missing_Startchar_Field.bdf_nested_startchar_before_endchar",
        "ftoutln.FT_Outline_Decompose.invalid_outline_or_interface_errors",
        "ftmm.FT_Set_Var_Blend_Coordinates.error_null_coords_with_nonzero_count",
        "ftmm.FT_Set_MM_Blend_Coordinates.error_null_coords_with_nonzero_count",
        "ftmm.FT_Set_MM_Design_Coordinates.error_null_coords_with_nonzero_count",
        "freetype.FT_New_Face.error_null_library_or_aface",
        "ftbdf.FT_Get_BDF_Property.error_missing_property_sets_none",
        "ftbdf.FT_Get_BDF_Property.error_null_face_or_output",
        "ftbdf.FT_Get_BDF_Property.error_unsupported_face_or_unselected_strike",
        "ftbdf.FT_Get_BDF_Charset_ID.error_non_bdf_face",
        "ftcolor.FT_COLOR_ROOT_TRANSFORM_MAX.invalid_runtime_behavior",
        "ftcolor.FT_COLR_PAINTFORMAT_UNSUPPORTED.invalid_format_returns_false",
        "ftcolor.FT_COLR_PAINT_FORMAT_MAX.read_paint_rejects_max_and_above",
        "ftcolor.FT_Get_Color_Glyph_ClipBox.null_and_non_sfnt_rejected",
        "ftcolor.FT_Get_Color_Glyph_ClipBox.malformed_clipbox_false_behavior",
        "ftcolor.FT_Get_Color_Glyph_Layer.invalid_inputs_rejected",
        "ftcolor.FT_Get_Color_Glyph_Layer.malformed_layer_record_false_behavior",
        "ftcolor.FT_Get_Color_Glyph_Paint.missing_or_invalid_root_returns_false",
        "ftcolor.FT_Get_Color_Glyph_Paint.non_null_opaque_paint_rejected",
        "ftcolor.FT_Get_Color_Glyph_Paint.null_and_non_sfnt_rejected",
        "ftcolor.FT_Get_Colorline_Stops.error_null_or_invalid_iterator",
        "ftcolor.FT_Get_Colorline_Stops.error_null_color_stop_policy",
        "ftcolor.FT_Get_Paint.error_null_or_missing_colr",
        "ftcolor.FT_Get_Paint.error_null_output_policy",
        "ftcolor.FT_Get_Paint_Layers.error_invalid_iterator_or_paint_offset",
        "ftcolor.FT_Get_Paint_Layers.error_null_arguments_policy",
        "ftcolor.FT_Palette_Data_Get.error_null_face_or_output",
        "ftcolor.FT_Palette_Select.error_null_face_or_invalid_palette_index",
        "ftcolor.FT_Palette_Set_Foreground_Color.error_null_face",
        "ftcid.FT_Get_CID_From_Glyph_Index.non_cid_or_null_face_errors_and_clears_output",
        "ftcid.FT_Get_CID_Is_Internally_CID_Keyed.non_cid_or_null_face_errors_and_clears_output",
        "ftcid.FT_Get_CID_Registry_Ordering_Supplement.error_non_cid_or_null_outputs",
        "ftcache.FTC_CMapCache_Lookup.error_null_cache_returns_zero",
        "ftcache.FTC_CMapCache_New.error_null_manager_or_output",
        "ftcache.FTC_ImageCache_Lookup.error_null_aglyph",
        "ftcache.FTC_ImageCache_Lookup.error_invalid_cache_type_face_or_glyph",
        "ftcache.FTC_ImageCache_LookupScaler.error_null_scaler_or_aglyph",
        "ftcache.FTC_ImageCache_New.error_null_manager_or_output",
        "ftcache.FTC_ImageCache_New.error_too_many_caches",
        "ftcache.FTC_Manager_LookupFace.error_null_output_or_manager",
        "ftcache.FTC_Manager_LookupFace.error_requester_failure",
        "fterrdef.FT_Err_Invalid_Cache_Handle.cache_lookup_rejects_null_manager",
        "ftcache.FTC_Manager_LookupSize.error_null_scaler_output_or_manager",
        "ftcache.FTC_Manager_LookupSize.error_requester_or_size_selection_failure",
        "ftcache.FTC_Manager_New.error_null_library",
        "ftcache.FTC_Manager_New.error_null_requester_or_output",
        "ftcache.FTC_SBitCache_LookupScaler.rejects_null_sbit_or_scaler",
        "ftcache.FTC_SBitCache_LookupScaler.clears_outputs_before_lookup",
        "ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output",
        "ftcache.FTC_SBitCache_Lookup.clears_outputs_before_lookup",
        "ftcache.FTC_SBitCache_New.error_outputs_null_cache",
        "ftcache.FTC_SBitCache_New.invalid_arguments_match_c",
        "freetype.FT_Open_Face.error_unknown_format_or_out_of_range_face",
        "ftgxval.FT_ClassicKern_Validate.rejects_invalid_arguments",
        "ftgxval.FT_ClassicKern_Validate.reports_unimplemented_or_invalid_table",
        "ftgxval.FT_VALIDATE_APPLE.absent_or_invalid_kern_table",
        "ftgxval.FT_VALIDATE_CKERN.malformed_table_error_matches_c",
        "ftglyph.FT_New_Glyph.error_null_library_or_output",
        "ftglyph.FT_New_Glyph.error_unsupported_format",
        "ftglyph.FT_New_Glyph.error_allocation_failure",
        "ftglyph.FT_Glyph_Transform.error_null_or_bad_glyph",
        "ftglyph.FT_Glyph_Transform.error_non_scalable_bitmap",
        "ftlist.FT_List_Iterate.stops_on_callback_error",
        "ftlist.FT_List_Iterate.null_list_or_iterator_error",
        "ftrender.FT_Set_Renderer.invalid_library_renderer_or_params",
        "ftrender.FT_Set_Renderer.set_mode_parameter_error_propagates",
        "ftsnames.FT_Get_Sfnt_LangTag.invalid_argument_errors",
        "ftstroke.FT_Stroker_New.invalid_library",
        "ftstroke.FT_Stroker_New.invalid_output_pointer",
        "ftstroke.FT_Stroker_New.allocation_failure",
        "ftstroke.FT_Stroker_ParseOutline.invalid_outline",
        "ftstroke.FT_Stroker_ParseOutline.invalid_stroker",
        "ftwinfnt.FT_Get_WinFNT_Header.null_face_returns_invalid_face_handle",
        "ftwinfnt.FT_Get_WinFNT_Header.null_output_returns_invalid_argument",
        "ftwinfnt.FT_Get_WinFNT_Header.non_winfnt_face_returns_invalid_argument",
        "ftoutln.FT_Outline_Check.invalid_null_or_count_mismatch",
        "fterrdef.FT_Err_Cannot_Render_Glyph.outline_raster_unsupported_mode_returns_error",
        "fterrdef.FT_Err_Cannot_Open_Resource.missing_path_returns_error",
        "fterrdef.FT_Err_Cannot_Open_Resource.resource_fork_open_failure_returns_error",
        "fterrdef.FT_Err_Cannot_Open_Stream.zero_length_file_returns_error",
        "fterrdef.FT_Err_Cannot_Open_Stream.resource_fork_stream_failure_returns_error",
        "fterrdef.FT_Err_Cannot_Render_Glyph.unsupported_render_mode_returns_error",
        "fterrdef.FT_Err_CMap_Table_Missing.sfnt_without_cmap_returns_error_where_required",
        "fterrdef.FT_Err_Hmtx_Table_Missing.incremental_metrics_exception_matches_c",
        "fterrdef.FT_Err_Horiz_Header_Missing.sfnt_missing_hhea_table",
        "fterrdef.FT_Err_Invalid_CodeRange.truetype_invalid_coderange",
        "fterrdef.FT_Err_Invalid_Frame_Operation.stream_frame_access_rejects_invalid_sequence",
        "fterrdef.FT_Err_Invalid_Frame_Read.stream_frame_bounds_rejected",
        "fterrdef.FT_Err_Invalid_Stream_Read.png_embedded_bitmap_read_failure",
        "fterrdef.FT_Err_Invalid_Stream_Handle.null_stream_rejected",
        "fterrdef.FT_Err_Invalid_Stream_Operation.stream_operation_failure",
        "fterrdef.FT_Err_Invalid_Stream_Seek.stream_seek_failure",
        "fterrdef.FT_Err_Invalid_Character_Code.char_index_name_lookup_invalid_code",
        "fterrdef.FT_Err_Invalid_Driver_Handle.module_driver_handle_validation",
        "fterrdef.FT_Err_Invalid_Outline.rust_invalid_outline_mapping",
        "fterrdef.FT_Err_Invalid_PPem.tt_size_reset_zero_ppem",
        "fterrdef.FT_Err_Invalid_CharMap_Format.sfnt_cmap_format_rejected",
        "fterrdef.FT_Err_Invalid_Horiz_Metrics.sfnt_hmtx_metrics_rejected",
        "fterrdef.FT_Err_Invalid_Offset.table_offset_bounds_rejected",
        "fterrdef.FT_Err_Invalid_Outline.malformed_outline_rejected",
        "fterrdef.FT_Err_Invalid_Stream_Skip.pcf_table_skip_failure",
        "fterrdef.FT_Err_Invalid_Slot_Handle.null_or_invalid_slot_rejected",
        "fterrdef.FT_Err_Invalid_Table.malformed_sfnt_table_rejected",
        "fterrdef.FT_Err_Invalid_SVG_Document.svg_document_failure_policy",
        "fterrdef.FT_Err_Locations_Missing.glyf_present_loca_missing",
        "fterrdef.FT_Err_Missing_Property.driver_property_unknown_name",
        "fterrdef.FT_Err_Invalid_Handle.generic_object_handle_validation",
        "fterrdef.FT_Err_Invalid_Pixel_Size.bitmap_strike_mismatch",
        "fterrdef.FT_Err_Nested_Frame_Access.stream_nested_frame_guard",
        "fterrdef.FT_Err_Array_Too_Large.allocator_growth_overflow_returns_error",
        "fterrdef.FT_Err_Bad_Argument.svg_preset_slot_bad_argument",
        "fterrdef.FT_Err_Ignore.parser_ignore_sentinel_not_public_success",
        "fterrdef.FT_Err_Missing_Module.no_driver_matches_face",
        "fterrdef.FT_Err_Out_Of_Memory.allocator_failure_injection",
        "fterrdef.FT_Err_Out_Of_Memory.cache_flush_then_oom",
        "fterrdef.FT_Err_Raster_Corrupted.bsdf_empty_contours_corrupted",
        "fterrdef.FT_Err_Raster_Corrupted.sdf_raster_missing_flag",
        "fterrdef.FT_Err_Raster_Negative_Height.monochrome_raster_negative_height",
        "fterrdef.FT_Err_Raster_Uninitialized.raster_render_without_pool",
        "fterrdef.FT_Err_Table_Missing.sfnt_required_table_missing",
        "fterrdef.FT_Err_Too_Many_Caches.cache_manager_cache_limit",
        "fterrdef.FT_Err_Unknown_File_Format.face_open_unknown_format",
        "fterrdef.FT_Err_Unknown_File_Format.malformed_container_probe_unknown",
        "fterrdef.FT_Err_Unimplemented_Feature.optional_module_feature_disabled",
        "ftdriver.FT_Prop_GlyphToScriptMap.invalid_face_error_matches_c",
        "ftdriver.FT_Prop_IncreaseXHeight.invalid_face_error_matches_c",
        "ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output",
        "ftoutln.FT_Outline_Copy.invalid_pointer_or_size_mismatch",
        "ftoutln.FT_Outline_Done.invalid_library_or_outline_errors",
        "ftoutln.FT_Outline_Embolden.invalid_or_indeterminate_orientation_errors",
        "ftoutln.FT_Outline_EmboldenXY.invalid_orientation_or_null_errors",
        "ftoutln.FT_Outline_New.invalid_arguments_and_limits",
        "ftoutln.FT_Orientation.geometry_fixture_matrix",
        "ftparams.FT_PARAM_TAG_LCD_FILTER_WEIGHTS.face_property_ignored",
        "ftimage.FT_Raster_New_Func.renderer_new_error_propagates",
        "ftimage.FT_Raster_Span_Func.missing_span_callback_errors",
        "ftimage.FT_Bitmap.invalid_target_buffer_errors",
        "ftimage.FT_PIXEL_MODE_NONE.invalid_render_target_errors",
        "ftimage.FT_RASTER_FLAG_DIRECT.mono_rejects_direct",
        "ftimage.FT_RASTER_FLAG_SDF.non_sdf_raster_rejects_sdf_shape",
        "ftimage.FT_Raster.null_raster_errors",
        "ftimage.FT_Raster_Funcs.render_callback_error_contract",
        "ftimage.FT_Raster_Params.invalid_param_errors",
        "ftincrem.FT_Incremental_FuncsRec.callback_error_propagates",
        "ftlcdfil.FT_Library_SetLcdFilter.unimplemented_without_subpixel_filtering",
        "ftlcdfil.FT_Library_SetLcdFilterWeights.unimplemented_without_subpixel_filtering",
        "ftlcdfil.FT_Library_SetLcdGeometry.unimplemented_with_subpixel_filtering",
        "ftmm.FT_Get_Multi_Master.null_output_error",
        "ftmm.FT_Get_Var_Axis_Flags.null_master_or_flags_error",
        "ftmm.FT_Get_Var_Axis_Flags.out_of_range_axis_error",
        "ftsnames.FT_Get_Sfnt_LangTag.format0_invalid_table_error",
        "tttables.FT_Sfnt_Table_Info.invalid_face_error",
        "tttables.TT_Postscript.invalid_post_format_error_runtime",
        "ftmoderr.FT_Mod_Err_Raster.prefixed_error_base",
        "ftmoderr.FT_Mod_Err_Sdf.prefixed_error_base",
        "ftmoderr.FT_Mod_Err_SFNT.prefixed_error_base",
        "ftmoderr.FT_Mod_Err_Smooth.prefixed_error_base",
        "ftmoderr.FT_Mod_Err_TrueType.prefixed_error_base",
        "ftmoderr.FT_Mod_Err_Type1.prefixed_error_base",
        "ftmoderr.FT_Mod_Err_Type42.prefixed_error_base",
        "ftmoderr.FT_Mod_Err_Winfonts.prefixed_error_base",
        "ftgxval.FT_TrueTypeGX_Validate.rejects_invalid_arguments",
        "ftgxval.FT_TrueTypeGX_Validate.reports_unimplemented_or_invalid_table",
        "ftgzip.FT_Gzip_Uncompress.rejects_invalid_arguments",
        "ftgzip.FT_Gzip_Uncompress.reports_buffer_too_small",
        "ftgzip.FT_Gzip_Uncompress.reports_invalid_compressed_data",
        "ftgzip.FT_Gzip_Uncompress.reports_unimplemented_without_zlib",
        "ftgzip.FT_Stream_OpenGzip.rejects_invalid_stream_handles",
        "ftgzip.FT_Stream_OpenGzip.rejects_invalid_gzip_header",
        "ftgzip.FT_Stream_OpenGzip.reports_unimplemented_without_zlib",
        "ftlzw.FT_Stream_OpenLZW.invalid_header_error",
        "ftlzw.FT_Stream_OpenLZW.null_stream_or_source_error",
        "ftlzw.FT_Stream_OpenLZW.unsupported_build_error",
        "ftotval.FT_OpenType_Validate.service_missing_error",
        "ftotval.FT_OpenType_Validate.malformed_table_error",
        "ftotval.FT_VALIDATE_GDEF.malformed_table_error",
        "ftotval.FT_VALIDATE_GPOS.malformed_table_error",
        "ftotval.FT_VALIDATE_GSUB.malformed_table_error",
        "ftotval.FT_VALIDATE_JSTF.absent_or_malformed_table",
        "ftotval.FT_VALIDATE_MATH.absent_or_malformed_table",
        "ftotval.FT_VALIDATE_OT.partial_failure_cleanup_contract",
        "ftpfr.FT_Get_PFR_Advance.non_pfr_returns_invalid_argument",
        "ftpfr.FT_Get_PFR_Advance.null_face_or_output_errors",
        "ftpfr.FT_Get_PFR_Kerning.null_face_or_vector_errors",
        "ftpfr.FT_Get_PFR_Metrics.non_pfr_outputs_valid_values_and_unknown_format",
        "ftpfr.FT_Get_PFR_Metrics.optional_outputs_and_null_face",
        "freetype.FT_Load_Glyph.matrix_load",
        "freetype.FT_Load_Glyph.error_out_of_range_null_face_or_invalid_flags",
        "freetype.FT_LOAD_FORCE_AUTOHINT.load_glyph_force_autohint_behavior",
        "freetype.FT_LOAD_PEDANTIC.pedantic_error_behavior",
        "freetype.FT_LOAD_TARGET_MODE.render_rejects_invalid_target_mode",
        "freetype.FT_New_Memory_Face.error_null_file_base",
        "freetype.FT_New_Memory_Face.error_null_library_or_aface",
        "freetype.FT_New_Memory_Face.error_bad_size_or_unknown_format",
        "freetype.FT_Open_Face.error_invalid_source_flags",
        "freetype.FT_Open_Face.error_null_library_args_or_aface",
        "freetype.FT_Render_Glyph.invalid_render_mode",
        "freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format",
        "freetype.FT_RENDER_MODE_MAX.render_glyph_rejects_sentinel",
        "fterrdef.FT_Err_Raster_Overflow.raster_buffer_or_cell_overflow",
        "ftimage.FT_RASTER_FLAG_AA.mono_rejects_aa",
        "ftstroke.FT_Stroker_BeginSubPath.invalid_arguments",
        "ftstroke.FT_Stroker_ConicTo.invalid_arguments",
        "ftstroke.FT_Stroker_CubicTo.invalid_arguments",
        "ftstroke.FT_Stroker_EndSubPath.invalid_stroker",
        "ftstroke.FT_Stroker_GetBorderCounts.invalid_stroker_or_border",
        "ftstroke.FT_Stroker_GetCounts.invalid_stroker",
        "ftstroke.FT_Glyph_Stroke.invalid_glyph_arguments",
        "ftstroke.FT_Glyph_Stroke.failure_sets_output_null_when_preserving_original",
        "ftstroke.FT_Glyph_StrokeBorder.invalid_glyph_arguments",
        "ftstroke.FT_Stroker_LineTo.invalid_arguments",
        "tttables.FT_Load_Sfnt_Table.missing_table_or_invalid_face_error",
        "tttables.FT_Sfnt_Table_Info.invalid_index_or_arguments",
    }


def ftstroke_export_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Stroker export rows needing real outline routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {
        "ftstroke.FT_STROKER_BORDER_LEFT.left_border_export_geometry": (
            "FT_STROKER_BORDER_LEFT export parity needs a maintained stroker "
            "border route proving left-border outline points, tags, contours, "
            "and orientation match pinned C"
        ),
        "ftstroke.FT_STROKER_BORDER_RIGHT.right_border_export_geometry": (
            "FT_STROKER_BORDER_RIGHT export parity needs a maintained stroker "
            "border route proving right-border outline points, tags, contours, "
            "and orientation match pinned C"
        ),
        "ftstroke.FT_StrokerBorder.border_selection_runtime_shape": (
            "FT_StrokerBorder selection parity needs a maintained route proving "
            "public border enum values select the same left/right border "
            "geometry and output shape as pinned C"
        ),
        "ftstroke.FT_Stroker_Export.exports_left_then_right": (
            "FT_Stroker_Export combined export parity needs a maintained route "
            "proving the combined outline appends left then right border "
            "geometry in pinned C point/tag/contour order"
        ),
        "ftstroke.FT_Stroker_Export.invalid_inputs_noop": (
            "FT_Stroker_Export invalid-input parity needs a maintained route "
            "proving null stroker or null outline inputs preserve the existing "
            "outline and return/no-op exactly like pinned C"
        ),
        "ftstroke.FT_Stroker_ExportBorder.valid_left_and_right_export": (
            "FT_Stroker_ExportBorder valid export parity needs a maintained "
            "route proving separate left and right border exports produce exact "
            "pinned C outline geometry"
        ),
        "ftstroke.FT_Stroker_ExportBorder.open_path_right_border_empty": (
            "FT_Stroker_ExportBorder open-path parity needs a maintained route "
            "proving the right border of an open path is empty or preserved "
            "exactly like pinned C"
        ),
        "ftstroke.FT_Stroker_ExportBorder.invalid_inputs_or_border_noop": (
            "FT_Stroker_ExportBorder invalid-input parity needs a maintained "
            "route proving invalid border values, null stroker, or null outline "
            "preserve output and no-op exactly like pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftstroke_curve_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Stroker conic/cubic curve rows needing real routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {
        "ftstroke.FT_Stroker_ConicTo.coincident_control_and_end_noop": (
            "FT_Stroker_ConicTo coincident-control parity needs a maintained "
            "route proving a control point equal to the current point and end "
            "point is treated as the same no-op or preserved-state case as "
            "pinned C"
        ),
        "ftstroke.FT_Stroker_CubicTo.coincident_controls_and_end_noop": (
            "FT_Stroker_CubicTo coincident-control parity needs a maintained "
            "route proving both controls and end point equal to the current "
            "point are treated as the same no-op or preserved-state case as "
            "pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftstroke_line_cap_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Stroker open-path cap rows needing real routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {
        "ftstroke.FT_STROKER_LINECAP_BUTT.butt_cap_open_line_geometry": (
            "FT_STROKER_LINECAP_BUTT parity needs a maintained open-path "
            "route proving butt caps terminate at the endpoint with the same "
            "border geometry, tags, and contours as pinned C"
        ),
        "ftstroke.FT_STROKER_LINECAP_ROUND.round_cap_open_line_geometry": (
            "FT_STROKER_LINECAP_ROUND parity needs a maintained open-path "
            "route proving round cap arc subdivision and emitted outline "
            "geometry match pinned C"
        ),
        "ftstroke.FT_STROKER_LINECAP_SQUARE.square_cap_open_line_geometry": (
            "FT_STROKER_LINECAP_SQUARE parity needs a maintained open-path "
            "route proving square cap endpoint extension and emitted outline "
            "geometry match pinned C"
        ),
        "ftstroke.FT_Stroker_LineCap.open_path_cap_geometry": (
            "FT_Stroker_LineCap runtime parity needs a maintained open-path "
            "route proving the public cap enum selects butt, round, and square "
            "geometry exactly like pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftstroke_path_construction_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Stroker BeginSubPath/LineTo rows needing real routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {
        "ftstroke.FT_Stroker_LineTo.line_segment_success": (
            "FT_Stroker_LineTo segment parity needs a maintained route proving "
            "line joins, emitted border points, tags, contours, and current "
            "point advancement match pinned C"
        ),
        "ftstroke.FT_Stroker_LineTo.first_segment_starts_subpath": (
            "FT_Stroker_LineTo first-segment parity needs a maintained route "
            "proving a line segment can initialize an otherwise empty subpath "
            "with the same border state and output geometry as pinned C"
        ),
        "ftstroke.FT_Stroker_LineTo.zero_length_line_noop": (
            "FT_Stroker_LineTo zero-length parity needs a maintained route "
            "proving a line to the current point preserves state or no-ops "
            "exactly like pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftstroke_line_join_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Stroker line-join rows needing real routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {
    }
    return exact_cases.get(row.case_id)


def ftstroke_outline_parse_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Stroker EndSubPath/ParseOutline rows needing routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {
        "ftstroke.FT_Stroker_ParseOutline.degenerate_contours_skipped": (
            "FT_Stroker_ParseOutline degenerate-contour parity needs a "
            "maintained route proving zero-length or malformed contours are "
            "skipped or preserved exactly like pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftstroke_count_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Stroker border/count rows needing real routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {
        "ftstroke.FT_Stroker_GetBorderCounts.closed_path_border_counts": (
            "FT_Stroker_GetBorderCounts closed-path parity needs a maintained "
            "route proving left/right point and contour counts after closing "
            "a path match pinned C"
        ),
        "ftstroke.FT_Stroker_GetBorderCounts.open_path_single_border_counts": (
            "FT_Stroker_GetBorderCounts open-path parity needs a maintained "
            "route proving open paths report the same single-border or empty "
            "border counts as pinned C"
        ),
        "ftstroke.FT_Stroker_GetBorderCounts.optional_output_pointers": (
            "FT_Stroker_GetBorderCounts optional-output parity needs a "
            "maintained route proving null output pointers are preserved while "
            "non-null outputs receive exactly the pinned-C counts"
        ),
        "ftstroke.FT_Stroker_GetCounts.combined_closed_path_counts": (
            "FT_Stroker_GetCounts closed-path parity needs a maintained route "
            "proving combined left/right point and contour totals match pinned C"
        ),
        "ftstroke.FT_Stroker_GetCounts.combined_open_path_counts": (
            "FT_Stroker_GetCounts open-path parity needs a maintained route "
            "proving combined open-path totals, including any empty-border "
            "handling, match pinned C"
        ),
        "ftstroke.FT_Stroker_GetCounts.optional_output_pointers": (
            "FT_Stroker_GetCounts optional-output parity needs a maintained "
            "route proving null output pointers are preserved while non-null "
            "outputs receive exactly the pinned-C combined counts"
        ),
    }
    return exact_cases.get(row.case_id)


def ftstroke_set_rewind_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Stroker_Set/Rewind rows needing real routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {}
    return exact_cases.get(row.case_id)


def ftstroke_glyph_stroke_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FT_Glyph_Stroke/StrokeBorder rows needing real routing."""
    if not row.operation.startswith("ftstroke."):
        return None
    exact_cases = {}
    return exact_cases.get(row.case_id)


def ftstroke_stroker_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for the stroker object/path subsystem that do not have a maintained route."""
    if not row.operation.startswith("ftstroke."):
        return None
    if ftstroke_null_noop_real_parity_reason(row):
        return None
    if row.case_id in {
        "ftstroke.FT_Stroker_New.valid_library_allocates_stroker",
        "ftstroke.FT_Stroker_Done.valid_stroker_releases_buffers",
        "ftstroke.FT_Stroker_Export.invalid_inputs_noop",
        "ftstroke.FT_Stroker_ExportBorder.invalid_inputs_or_border_noop",
    }:
        return None
    if operation_is_compile_contract(row.operation):
        return None
    if row.operation in {
        "ftstroke.outline_get_inside_border",
        "ftstroke.outline_get_outside_border",
        "ftstroke.outline_border_orientation_pair",
    }:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    export_pending = ftstroke_export_pending_reason(row)
    if export_pending:
        return export_pending
    curve_pending = ftstroke_curve_pending_reason(row)
    if curve_pending:
        return curve_pending
    line_cap_pending = ftstroke_line_cap_pending_reason(row)
    if line_cap_pending:
        return line_cap_pending
    path_pending = ftstroke_path_construction_pending_reason(row)
    if path_pending:
        return path_pending
    line_join_pending = ftstroke_line_join_pending_reason(row)
    if line_join_pending:
        return line_join_pending
    outline_parse_pending = ftstroke_outline_parse_pending_reason(row)
    if outline_parse_pending:
        return outline_parse_pending
    count_pending = ftstroke_count_pending_reason(row)
    if count_pending:
        return count_pending
    set_rewind_pending = ftstroke_set_rewind_pending_reason(row)
    if set_rewind_pending:
        return set_rewind_pending
    glyph_stroke_pending = ftstroke_glyph_stroke_pending_reason(row)
    if glyph_stroke_pending:
        return glyph_stroke_pending
    pending_case_groups = {
        ("ftstroke.FT_Stroker_New.valid_library_allocates_stroker",): (
            "FT_Stroker allocation/lifecycle parity needs a maintained "
            "non-null stroker object route proving library allocation, "
            "attribute storage, owned border buffers, and final cleanup across "
            "pinned C, Rust FFI, C ABI, and WASM ABI"
        ),
        ("ftstroke.FT_Stroker_Done.valid_stroker_releases_buffers",): (
            "FT_Stroker_Done parity needs a maintained non-null stroker route "
            "proving owned buffers are released after normal use and after "
            "export without double-free or leaked observable state"
        ),
    }
    for case_ids, reason in pending_case_groups.items():
        if row.case_id in case_ids:
            return reason
    return None


def ftstroke_null_noop_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact null-stroker no-op routes verified against FreeType ftstroke.c."""
    if row.case_id in {
        "ftstroke.FT_Stroker_Set.null_stroker_noop",
        "ftstroke.FT_Stroker_Rewind.null_stroker_noop",
        "ftstroke.FT_Stroker_Done.null_stroker_noop",
    }:
        return (
            "FreeType ftstroke.c null-stroker no-op with explicit C oracle, "
            "Rust FFI, C ABI, and WASM route"
        )
    return None


def ftstroke_zero_line_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact zero-length LineTo route verified against FreeType ftstroke.c."""
    if row.operation != "ftstroke.line_to":
        return None
    if row.case_id == "ftstroke.FT_Stroker_LineTo.zero_length_line_noop":
        return (
            "FT_Stroker_LineTo zero-length no-op validates through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI; full non-zero stroker "
            "geometry remains pending"
        )
    if row.case_id == "ftstroke.FT_Stroker_LineTo.pre_end_counts_invalid_outline":
        return (
            "FT_Stroker_LineTo first non-zero segment pre-EndSubPath count "
            "error validates through pinned C oracle, Rust FFI, C ABI, and "
            "WASM ABI; finalized counts and exported border geometry remain "
            "pending"
        )
    if row.case_id == "ftstroke.FT_Stroker_LineTo.line_segment_success":
        return (
            "FT_Stroker_LineTo closed horizontal segment finalization and "
            "exported outline geometry validate through pinned C oracle, Rust "
            "FFI, C ABI, and WASM ABI; general line joins and curve stroking "
            "remain pending"
        )
    if row.case_id == "ftstroke.FT_Stroker_LineTo.first_segment_starts_subpath":
        return (
            "FT_Stroker_LineTo first-segment BeginSubPath/LineTo/EndSubPath "
            "status sequence and exported open-line outline validate through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI; general line joins "
            "and curve stroking remain pending"
        )
    return None


def ftstroke_degenerate_curve_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact coincident conic/cubic no-op routes verified against ftstroke.c."""
    if row.case_id in {
        "ftstroke.FT_Stroker_ConicTo.coincident_control_and_end_noop",
        "ftstroke.FT_Stroker_CubicTo.coincident_controls_and_end_noop",
    }:
        return (
            "FT_Stroker conic/cubic coincident-control no-op validates through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI; full curve "
            "geometry remains pending"
        )
    return None


def ftstroke_parse_line_conic_cubic_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact mixed line/conic/cubic ParseOutline route verified against ftstroke.c."""
    if (
        row.operation == "ftstroke.parse_outline"
        and row.case_id == "ftstroke.FT_Stroker_ParseOutline.line_conic_cubic_success"
    ):
        return (
            "FT_Stroker_ParseOutline mixed line, implied-conic, cubic, and "
            "font-like outline rows validate parse status, operation order, "
            "counts, exported geometry, and cbox through pinned C oracle, Rust "
            "FFI, C ABI, and WASM ABI"
        )
    return None


def ftstroke_set_attribute_geometry_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact FT_Stroker_Set attribute matrix route verified against ftstroke.c."""
    if (
        row.operation == "ftstroke.set"
        and row.case_id == "ftstroke.FT_Stroker_Set.attributes_affect_geometry"
    ):
        return (
            "FT_Stroker_Set radius, line cap, line join, and miter-limit fields "
            "validate through a bounded line/corner/conic/cubic matrix against "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    return None


def ftstroke_parse_degenerate_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact degenerate ParseOutline no-op route verified against ftstroke.c."""
    if (
        row.operation == "ftstroke.parse_outline"
        and row.case_id
        in {
            "ftstroke.FT_Stroker_ParseOutline.degenerate_single_point_and_empty_noop",
            "ftstroke.FT_Stroker_ParseOutline.degenerate_contours_skipped",
            "ftstroke.FT_Stroker_EndSubPath.no_segment_after_begin",
        }
    ):
        if row.case_id == "ftstroke.FT_Stroker_EndSubPath.no_segment_after_begin":
            return (
                "FT_Stroker_ParseOutline degenerate single-point and empty contour "
                "rows validate parse status and zero counts through pinned C "
                "oracle, Rust FFI, C ABI, and WASM ABI; direct EndSubPath "
                "no-segment counts are intentionally not promoted because the "
                "pinned C build segfaults after that direct state"
            )
        return (
            "FT_Stroker_ParseOutline empty, single-point, and mixed "
            "degenerate-plus-valid contour behavior validates through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI; full parse/finalization/export "
            "geometry remains pending"
        )
    return None


def ftstroke_end_no_segment_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact direct EndSubPath no-segment status route verified against ftstroke.c."""
    if (
        row.operation == "ftstroke.end_subpath"
        and row.case_id == "ftstroke.FT_Stroker_EndSubPath.no_segment_status_only"
    ):
        return (
            "FT_Stroker_EndSubPath closed no-segment status validates through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI; counts after this "
            "direct state are not promoted because the pinned C build segfaults"
        )
    return None


def ftstroke_finalized_counts_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact finalized count routes for simple open/closed stroker paths."""
    exact_cases = {
        "ftstroke.FT_Stroker_GetCounts.combined_closed_path_counts",
        "ftstroke.FT_Stroker_GetCounts.combined_open_path_counts",
        "ftstroke.FT_Stroker_GetCounts.optional_output_pointers",
        "ftstroke.FT_Stroker_GetBorderCounts.closed_path_border_counts",
        "ftstroke.FT_Stroker_GetBorderCounts.open_path_single_border_counts",
        "ftstroke.FT_Stroker_GetBorderCounts.optional_output_pointers",
    }
    if row.case_id in exact_cases and row.operation in {
        "ftstroke.get_counts",
        "ftstroke.get_border_counts",
    }:
        return (
            "FT_Stroker finalized open/closed public count outputs validate "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI; exported "
            "outline geometry remains pending"
        )
    return None


def ftstroke_reset_counts_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact Set/Rewind path-clearing routes proven through public counts."""
    exact_cases = {
        "ftstroke.FT_Stroker_Set.clears_existing_path",
        "ftstroke.FT_Stroker_Rewind.clears_previous_path",
        "ftstroke.FT_Stroker_Rewind.set_calls_rewind",
    }
    if row.case_id in exact_cases and row.operation in {
        "ftstroke.set",
        "ftstroke.rewind",
        "ftstroke.set_then_rewind_observed",
    }:
        return (
            "FT_Stroker Set/Rewind path-clearing count outputs validate "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI; attribute "
            "geometry remains pending"
        )
    return None


def ftstroke_rewind_attributes_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact Rewind route proving attributes survive path clearing."""
    if (
        row.case_id == "ftstroke.FT_Stroker_Rewind.attributes_preserved"
        and row.operation == "ftstroke.rewind"
    ):
        return (
            "FT_Stroker_Rewind preserves radius, square cap, fixed-miter join, "
            "and miter-limit attributes while clearing path state; the second "
            "acute-corner path validates exact exported outline geometry and "
            "counts through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    return None


def ftstroke_set_miter_limit_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact Set route proving miter limits below one are clamped."""
    if (
        row.case_id == "ftstroke.FT_Stroker_Set.miter_limit_clamped_to_one"
        and row.operation == "ftstroke.set"
    ):
        return (
            "FT_Stroker_Set clamps miter_limit values below 1.0 (65536) before "
            "fixed-miter fallback decisions; five rows validate effective "
            "limits and exact exported outline geometry through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        )
    return None


def ftstroke_miter_join_real_parity_reason(row: ConcreteInput) -> str | None:
    """Exact fixed/variable miter join geometry routes."""
    exact_cases = {
        "ftstroke.FT_STROKER_LINEJOIN_MITER.alias_matches_variable_join_geometry": (
            "FT_STROKER_LINEJOIN_MITER alias geometry validates that the public "
            "alias selects the same exported outline as "
            "FT_STROKER_LINEJOIN_MITER_VARIABLE through pinned C oracle, Rust "
            "FFI, C ABI, and WASM ABI"
        ),
        "ftstroke.FT_STROKER_LINEJOIN_MITER_FIXED.fixed_miter_limit_geometry": (
            "FT_STROKER_LINEJOIN_MITER_FIXED exported geometry validates "
            "fixed-miter fallback and longer-miter output for limits 65536 "
            "and 131072 through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftstroke.FT_STROKER_LINEJOIN_MITER_VARIABLE.variable_miter_limit_geometry": (
            "FT_STROKER_LINEJOIN_MITER_VARIABLE exported geometry validates "
            "variable clipped-miter and longer-miter output for limits 65536 "
            "and 131072 through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftstroke.FT_STROKER_LINEJOIN_BEVEL.bevel_join_geometry": (
            "FT_STROKER_LINEJOIN_BEVEL exported geometry validates bevel "
            "points, tags, contours, and join-shape output through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftstroke.FT_STROKER_LINEJOIN_ROUND.round_join_geometry": (
            "FT_STROKER_LINEJOIN_ROUND manual path geometry validates line, "
            "conic, and cubic round-join output from maintained path-record "
            "fixtures through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftstroke.FT_STROKER_LINEJOIN_ROUND.wide_curve_join_restoration": (
            "FT_STROKER_LINEJOIN_ROUND wide-curve route validates saved "
            "non-round join restoration after curve subdivision through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftstroke.FT_Stroker_LineJoin.join_geometry_and_miter_limit": (
            "FT_Stroker_LineJoin matrix validates all public join enum values "
            "and miter-limit inputs select the same exported geometry through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        ),
    }
    if row.operation in {
        "ftstroke.join_geometry",
        "ftstroke.join_geometry_alias",
        "ftstroke.stroke_manual_path",
        "ftstroke.stroke_wide_curve",
    }:
        return exact_cases.get(row.case_id)
    return None


def ftcache_image_lookup_scaler_pending_reason(row: ConcreteInput) -> str | None:
    """Case- and variant-specific FTC_ImageCache_LookupScaler pending rows."""
    if row.operation != "ftcache.image_cache_lookup_scaler":
        return None
    variant = row.variant_id or "single"
    load_flags = row.params.get("load_flags", "unspecified")
    variant_detail = f"variant {variant} load_flags={load_flags}"
    if (
        row.case_id
        == "ftcache.FTC_ImageCache_LookupScaler.planned_cache_subsystem_not_out_of_scope"
    ):
        return (
            "FTC_ImageCache_LookupScaler planned-cache route is in scope but "
            f"pending for {variant_detail}; exact parity needs a maintained "
            "manager-owned image cache, scaler lookup, and same-input oracle "
            "comparison instead of treating cache lookup as out of scope"
        )
    if (
        row.case_id
        == "ftcache.FTC_ImageCache_LookupScaler.success_pixel_and_point_scalers"
    ):
        return (
            "FTC_ImageCache_LookupScaler pixel/point scaler parity needs a "
            f"maintained route for {variant_detail} proving integer pixel "
            "sizes and 26.6 point sizes with x/y resolution select the same "
            "FT_Size metrics and glyph output as pinned C"
        )
    if (
        row.case_id
        == "ftcache.FTC_ImageCache_LookupScaler.success_lookup_hit_miss_and_repeated"
    ):
        return (
            "FTC_ImageCache_LookupScaler hit/miss/repeat parity needs a "
            f"maintained route for {variant_detail} proving first lookup, "
            "repeat lookup, and missing glyph behavior match pinned C cache "
            "node identity and glyph output"
        )
    if (
        row.case_id
        == "ftcache.FTC_ImageCache_LookupScaler.success_node_acquire_and_unref"
    ):
        return (
            "FTC_ImageCache_LookupScaler node lifecycle parity needs a "
            f"maintained route for {variant_detail} proving anode acquisition, "
            "FTC_Node_Unref release, and post-unref cache state match pinned C"
        )
    if (
        row.case_id
        == "ftcache.FTC_ImageCache_LookupScaler.load_flags_truncation_policy"
    ):
        return (
            "FTC_ImageCache_LookupScaler load-flag truncation parity needs a "
            f"maintained route for {variant_detail} proving FT_ULong input is "
            "truncated to the pinned C signed load_flags path before lookup"
        )
    return None


def ftcache_cmap_lookup_pending_reason(row: ConcreteInput) -> str | None:
    """Case- and codepoint-specific FTC_CMapCache_Lookup pending rows."""
    if row.operation != "ftcache.cmap_cache_lookup":
        return None
    variant = row.variant_id or "single"
    codepoint = variant.removeprefix("cp") if variant.startswith("cp") else variant
    variant_detail = f"variant {variant} codepoint={codepoint}"
    if row.case_id in {
        "ftcache.FTC_CMapCache_Lookup.planned_cache_subsystem_not_out_of_scope",
        "ftcache.FTC_CMapCache_Lookup.success_lookup_hit_and_repeat_hit",
        "ftcache.FTC_CMapCache_Lookup.success_lookup_miss_returns_zero",
        "ftcache.FTC_CMapCache_Lookup.success_negative_cmap_index_uses_current_charmap",
        "ftcache.FTC_CMapCache_Lookup.lifecycle_remove_faceid_and_reset",
    }:
        return None
    if (
        row.case_id
        == "ftcache.FTC_CMapCache_Lookup.planned_cache_subsystem_not_out_of_scope"
    ):
        return (
            "FTC_CMapCache_Lookup planned-cache route is in scope but pending "
            f"for {variant_detail}; exact parity needs a maintained manager-owned "
            "CMap cache and same-input oracle comparison instead of treating "
            "cache lookup as out of scope"
        )
    if row.case_id == "ftcache.FTC_CMapCache_Lookup.success_lookup_hit_and_repeat_hit":
        return (
            "FTC_CMapCache_Lookup hit/repeat parity needs a maintained route "
            f"for {variant_detail} proving first lookup, repeat lookup, glyph "
            "index output, requester use, and cache identity match pinned C"
        )
    if row.case_id == "ftcache.FTC_CMapCache_Lookup.success_lookup_miss_returns_zero":
        return (
            "FTC_CMapCache_Lookup miss parity needs a maintained route for "
            f"{variant_detail} proving missing character lookup returns exactly "
            "zero without corrupting cache state like pinned C"
        )
    if (
        row.case_id
        == "ftcache.FTC_CMapCache_Lookup.success_negative_cmap_index_uses_current_charmap"
    ):
        return (
            "FTC_CMapCache_Lookup negative-cmap-index parity needs a maintained "
            f"route for {variant_detail} proving cmap_index=-1 uses the face's "
            "current charmap and matches pinned C glyph index output"
        )
    if row.case_id == "ftcache.FTC_CMapCache_Lookup.lifecycle_remove_faceid_and_reset":
        return (
            "FTC_CMapCache_Lookup remove-face/reset lifecycle parity needs a "
            f"maintained route for {variant_detail} proving cache entries are "
            "evicted or rebuilt after FTC_Manager_RemoveFaceID and manager reset "
            "exactly like pinned C"
        )
    return None


def ftcache_manager_lookup_size_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FTC_Manager_LookupSize/Scaler pending rows."""
    if not row.operation.startswith("ftcache."):
        return None
    exact_cases = {
        "ftcache.FTC_Manager_LookupSize.planned_cache_subsystem_not_out_of_scope": (
            "FTC_Manager_LookupSize planning parity needs a maintained "
            "same-input cache manager route instead of treating scaler-based "
            "size lookup as out of scope"
        ),
        "ftcache.FTC_Manager_LookupSize.success_pixel_size_scaler": (
            "FTC_Manager_LookupSize pixel-scaler parity needs a maintained "
            "route proving width/height pixel sizes select the same FT_Size "
            "metrics and cached size handle as pinned C"
        ),
        "ftcache.FTC_Manager_LookupSize.success_point_size_resolution_scaler": (
            "FTC_Manager_LookupSize point-scaler parity needs a maintained "
            "route proving 26.6 point sizes plus x/y resolution select the "
            "same FT_Size metrics and cached size handle as pinned C"
        ),
        "ftcache.FTC_Manager_LookupSize.success_repeat_lookup_cached_size": (
            "FTC_Manager_LookupSize repeat-lookup parity needs a maintained "
            "route proving repeated scaler lookup returns the same cached size "
            "identity and output fields as pinned C"
        ),
        "ftcache.FTC_ScalerRec.pixel_scaler_uses_integer_pixels": (
            "FTC_ScalerRec pixel parity needs a maintained route proving "
            "pixel=1 interprets width/height as integer pixel sizes exactly "
            "like pinned C"
        ),
        "ftcache.FTC_ScalerRec.point_scaler_uses_26_6_points_and_resolution": (
            "FTC_ScalerRec point parity needs a maintained route proving "
            "pixel=0 interprets width/height as 26.6 point sizes with x/y "
            "resolution exactly like pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcache_manager_new_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FTC_Manager_New/FTC_Manager pending rows."""
    if not row.operation.startswith("ftcache."):
        return None
    exact_cases = {
        "ftcache.FTC_Manager_New.planned_cache_subsystem_not_out_of_scope": (
            "FTC_Manager_New planning parity needs a maintained same-input "
            "cache-manager route instead of treating the public FTC manager "
            "subsystem as out of scope"
        ),
        "ftcache.FTC_Manager_New.success_defaults_for_zero_limits": (
            "FTC_Manager_New default-limit parity needs a maintained route "
            "proving zero max_faces, max_sizes, and max_bytes inputs select "
            "the same pinned-C default cache limits"
        ),
        "ftcache.FTC_Manager_New.success_custom_limits_and_req_data": (
            "FTC_Manager_New custom-limit parity needs a maintained route "
            "proving explicit limits and requester data are stored, forwarded, "
            "and reported through lookups exactly like pinned C"
        ),
        "ftcache.FTC_Manager_New.lifecycle_create_lookup_reset_done": (
            "FTC_Manager_New lifecycle parity needs a maintained route proving "
            "create, lookup, reset, and done sequencing produces the same "
            "observable cache state and return codes as pinned C"
        ),
        "ftcache.FTC_Manager.reset_and_done_lifecycle": (
            "FTC_Manager reset/done parity needs a maintained route proving "
            "reset clears cached faces, sizes, caches, and nodes while done "
            "tears down ownership exactly like pinned C"
        ),
        "ftcache.FTC_Manager.owns_faces_sizes_and_cache_nodes": (
            "FTC_Manager ownership parity needs a maintained route proving "
            "manager-owned face, size, cache, and node lifetimes match pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcache_image_lookup_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FTC_ImageCache_Lookup/FTC_ImageType pending rows."""
    if not row.operation.startswith("ftcache."):
        return None
    exact_cases = {
        "ftcache.FTC_ImageCache_Lookup.planned_cache_subsystem_not_out_of_scope": (
            "FTC_ImageCache_Lookup planning parity needs a maintained "
            "same-input image-cache route instead of treating cache lookup as "
            "out of scope"
        ),
        "ftcache.FTC_ImageCache_Lookup.success_lookup_hit_and_repeat_hit": (
            "FTC_ImageCache_Lookup hit/repeat parity needs a maintained route "
            "proving first lookup, repeat lookup, glyph output, requester use, "
            "and cache identity match pinned C"
        ),
        "ftcache.FTC_ImageCache_Lookup.success_node_acquire_and_unref": (
            "FTC_ImageCache_Lookup node lifecycle parity needs a maintained "
            "route proving anode acquisition, FTC_Node_Unref release, and "
            "post-unref cache state match pinned C"
        ),
        "ftcache.FTC_ImageCache_Lookup.success_null_anode_ephemeral_glyph": (
            "FTC_ImageCache_Lookup null-anode parity needs a maintained route "
            "proving a null anode returns an ephemeral glyph with the same "
            "ownership and cache-node side effects as pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcache_manager_done_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FTC_Manager_Done pending rows."""
    if not row.operation.startswith("ftcache."):
        return None
    exact_cases = {
        "ftcache.FTC_Manager_Done.planned_cache_subsystem_not_out_of_scope": (
            "FTC_Manager_Done planning parity needs a maintained same-input "
            "manager teardown route instead of treating FTC manager destruction "
            "as out of scope"
        ),
        "ftcache.FTC_Manager_Done.success_destroy_empty_manager": (
            "FTC_Manager_Done empty-manager parity needs a maintained route "
            "proving an empty manager releases ownership and returns/no-ops "
            "exactly like pinned C"
        ),
        "ftcache.FTC_Manager_Done.success_destroy_populated_manager": (
            "FTC_Manager_Done populated-manager parity needs a maintained "
            "route proving cached faces, sizes, caches, and nodes are destroyed "
            "in the same observable order and ownership state as pinned C"
        ),
        "ftcache.FTC_Manager_Done.success_null_or_invalid_library_noop": (
            "FTC_Manager_Done null/foreign-library parity needs a maintained "
            "route proving null managers and foreign-library ownership cases "
            "return or no-op exactly like pinned C"
        ),
        "ftcache.FTC_Manager_Done.node_reference_lifecycle_on_done": (
            "FTC_Manager_Done referenced-node parity needs a maintained route "
            "proving referenced nodes during manager teardown keep or release "
            "observable cache ownership exactly like pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcache_manager_remove_faceid_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FTC_Manager_RemoveFaceID pending rows."""
    if not row.operation.startswith("ftcache."):
        return None
    exact_cases = {
        "ftcache.FTC_Manager_RemoveFaceID.planned_cache_subsystem_not_out_of_scope": (
            "FTC_Manager_RemoveFaceID planning parity needs a maintained "
            "same-input face-id eviction route instead of treating cache "
            "eviction as out of scope"
        ),
        "ftcache.FTC_Manager_RemoveFaceID.success_removes_unreferenced_face_size_and_nodes": (
            "FTC_Manager_RemoveFaceID unreferenced eviction parity needs a "
            "maintained route proving unreferenced face, size, and node entries "
            "for the face ID are removed exactly like pinned C"
        ),
        "ftcache.FTC_Manager_RemoveFaceID.success_referenced_nodes_hidden_until_unref": (
            "FTC_Manager_RemoveFaceID referenced-node parity needs a maintained "
            "route proving referenced nodes are hidden from future lookup until "
            "FTC_Node_Unref releases them exactly like pinned C"
        ),
        "ftcache.FTC_Manager_RemoveFaceID.success_other_face_ids_unchanged": (
            "FTC_Manager_RemoveFaceID isolation parity needs a maintained route "
            "proving eviction of one face ID preserves cached faces, sizes, and "
            "nodes for other face IDs exactly like pinned C"
        ),
        "ftcache.FTC_Manager_RemoveFaceID.success_null_manager_noop": (
            "FTC_Manager_RemoveFaceID null-manager parity needs a maintained "
            "route proving a null manager returns or no-ops exactly like "
            "pinned C"
        ),
        "ftcache.FTC_Manager_RemoveFaceID.success_null_or_unknown_face_id": (
            "FTC_Manager_RemoveFaceID null/unknown-face parity needs a "
            "maintained route proving null or unknown face IDs leave cache "
            "state unchanged exactly like pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcache_manager_lookup_face_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FTC_Manager_LookupFace/FTC_FaceID pending rows."""
    if not row.operation.startswith("ftcache."):
        return None
    exact_cases = {
        "ftcache.FTC_Manager_LookupFace.planned_cache_subsystem_not_out_of_scope": (
            "FTC_Manager_LookupFace planning parity needs a maintained "
            "same-input requester route instead of treating face lookup as "
            "out of scope"
        ),
        "ftcache.FTC_Manager_LookupFace.success_first_lookup_invokes_requester": (
            "FTC_Manager_LookupFace first-lookup parity needs a maintained "
            "route proving the first lookup invokes the requester once with "
            "the same FTC_FaceID and requester data as pinned C"
        ),
        "ftcache.FTC_Manager_LookupFace.success_repeat_lookup_returns_cached_face": (
            "FTC_Manager_LookupFace repeat-lookup parity needs a maintained "
            "route proving repeated lookup returns the cached face without "
            "reinvoking the requester, matching pinned C handle identity"
        ),
        "ftcache.FTC_Manager_LookupFace.success_face_has_no_required_current_size": (
            "FTC_Manager_LookupFace current-size parity needs a maintained "
            "route proving the returned face has no required current size "
            "unless the requester or later size lookup establishes one"
        ),
        "ftcache.FTC_FaceID.pointer_identity_key": (
            "FTC_FaceID pointer-identity parity needs a maintained requester "
            "route proving pointer identity, not pointed-to bytes or fallback "
            "string equality, is the cache key exactly like pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcache_sbit_lookup_scaler_pending_reason(row: ConcreteInput) -> str | None:
    """Case- and variant-specific FTC_SBitCache_LookupScaler pending rows."""
    if row.operation != "ftcache.sbit_cache_lookup_scaler":
        return None
    if (
        row.case_id
        in {
            "ftcache.FTC_SBitCache_LookupScaler.scaler_size_semantics_match_c",
            "ftcache.FTC_SBitCache_LookupScaler.load_flags_truncate_to_int32",
        }
        and unresolved_assets_reason(row) is None
    ):
        return None
    runtime = runtime_id(row)
    variant = runtime.split("@", 1)[1] if "@" in runtime else "single"
    if row.case_id == "ftcache.FTC_SBitCache_LookupScaler.scaler_size_semantics_match_c":
        return (
            "FTC_SBitCache_LookupScaler scaler-size parity needs a maintained "
            f"route for variant {variant} proving pixel and point scaler "
            "width/height/resolution semantics select the same strike, size "
            "metrics, and sbit output as pinned C"
        )
    if row.case_id == "ftcache.FTC_SBitCache_LookupScaler.load_flags_truncate_to_int32":
        return (
            "FTC_SBitCache_LookupScaler load-flag parity needs a maintained "
            f"route for variant {variant} proving FT_ULong load_flags are "
            "truncated to the pinned C signed 32-bit path before sbit lookup"
        )
    return None


def ftcache_node_lifecycle_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific FTC_Node/FTC_Node_Unref pending rows."""
    if not row.operation.startswith("ftcache."):
        return None
    exact_cases = {
    }
    return exact_cases.get(row.case_id)


def ftcache_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for the cache subsystem that do not have a maintained success route."""
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    image_lookup_scaler_pending = ftcache_image_lookup_scaler_pending_reason(row)
    if image_lookup_scaler_pending:
        return image_lookup_scaler_pending
    cmap_lookup_pending = ftcache_cmap_lookup_pending_reason(row)
    if cmap_lookup_pending:
        return cmap_lookup_pending
    manager_lookup_size_pending = ftcache_manager_lookup_size_pending_reason(row)
    if manager_lookup_size_pending:
        return manager_lookup_size_pending
    manager_new_pending = ftcache_manager_new_pending_reason(row)
    if manager_new_pending:
        return manager_new_pending
    image_lookup_pending = ftcache_image_lookup_pending_reason(row)
    if image_lookup_pending:
        return image_lookup_pending
    manager_done_pending = ftcache_manager_done_pending_reason(row)
    if manager_done_pending:
        return manager_done_pending
    remove_faceid_pending = ftcache_manager_remove_faceid_pending_reason(row)
    if remove_faceid_pending:
        return remove_faceid_pending
    lookup_face_pending = ftcache_manager_lookup_face_pending_reason(row)
    if lookup_face_pending:
        return lookup_face_pending
    sbit_lookup_scaler_pending = ftcache_sbit_lookup_scaler_pending_reason(row)
    if sbit_lookup_scaler_pending:
        return sbit_lookup_scaler_pending
    node_lifecycle_pending = ftcache_node_lifecycle_pending_reason(row)
    if node_lifecycle_pending:
        return node_lifecycle_pending
    cache_creation_exact_cases = {
        "ftcache.FTC_CMapCache.manager_owned_opaque_cache": (
            "FTC_CMapCache opaque-handle parity needs a maintained cache route "
            "proving the public CMap cache handle is manager-owned, stable "
            "across lookups, and not a caller-owned descriptor"
        ),
        "ftcache.FTC_ImageCache.manager_owned_opaque_cache": (
            "FTC_ImageCache opaque-handle parity needs a maintained cache route "
            "proving the public Image cache handle is manager-owned, stable "
            "across glyph lookups, and participates in node ownership like "
            "pinned C"
        ),
        "ftcache.FTC_SBitCache.manager_owned_sbit_cache": (
            "FTC_SBitCache opaque-handle parity needs a maintained cache route "
            "proving the public SBit cache handle is manager-owned, stable "
            "across sbit lookups, and participates in node ownership like "
            "pinned C"
        ),
        "ftcache.FTC_CMapCache_New.planned_cache_subsystem_not_out_of_scope": (
            "FTC_CMapCache_New planning parity needs a maintained same-input "
            "CMap cache creation route instead of treating the public FTC cache "
            "subsystem as out of scope"
        ),
        "ftcache.FTC_CMapCache_New.success_create_and_destroy_with_manager": (
            "FTC_CMapCache_New lifecycle parity needs a maintained route proving "
            "a manager-created CMap cache is destroyed through FTC_Manager_Done "
            "with the same ownership side effects as pinned C"
        ),
        "ftcache.FTC_CMapCache_New.lifecycle_after_manager_reset": (
            "FTC_CMapCache_New reset parity needs a maintained route proving "
            "manager reset preserves the cache handle while clearing cached CMap "
            "entries exactly like pinned C"
        ),
        "ftcache.FTC_ImageCache_New.planned_cache_subsystem_not_out_of_scope": (
            "FTC_ImageCache_New planning parity needs a maintained same-input "
            "Image cache creation route instead of treating the public FTC cache "
            "subsystem as out of scope"
        ),
        "ftcache.FTC_ImageCache_New.success_create_lookup_destroy_lifecycle": (
            "FTC_ImageCache_New lookup lifecycle parity needs a maintained route "
            "proving create, glyph lookup, node ownership, and manager-driven "
            "destroy behavior match pinned C"
        ),
        "ftcache.FTC_ImageCache_New.success_manager_reset_preserves_handle": (
            "FTC_ImageCache_New reset parity needs a maintained route proving "
            "manager reset preserves the Image cache handle while clearing "
            "cached glyph and node state like pinned C"
        ),
        "ftcache.FTC_SBitCache_New.creates_manager_owned_cache": (
            "FTC_SBitCache_New creation parity needs a maintained route proving "
            "the created SBit cache is manager-owned, supports lookup/node "
            "lifecycle, and is destroyed through manager teardown like pinned C"
        ),
    }
    if row.case_id in cache_creation_exact_cases:
        return cache_creation_exact_cases[row.case_id]
    return None


def ftcolor_composite_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific COLR composite rows that need maintained paint graph routing."""
    if not row.operation.startswith("ftcolor."):
        return None
    exact_cases = {
        "ftcolor.FT_PaintComposite.get_paint_composite_values": (
            "FT_PaintComposite parity needs a maintained FT_Get_Paint route "
            "proving source paint, backdrop paint, and composite mode fields "
            "match pinned C public union output"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_COMPOSITE.paint_composite_payload": (
            "FT_COLR_PAINTFORMAT_COMPOSITE parity needs a maintained paint node "
            "route proving composite payload shape and nested opaque paint "
            "handles match pinned C"
        ),
        "ftcolor.FT_Composite_Mode.paint_composite_modes_runtime": (
            "FT_Composite_Mode parity needs a maintained traversal route proving "
            "every public composite enum value is emitted from valid COLR paint "
            "graphs with exact pinned C numeric values"
        ),
        "ftcolor.FT_COLR_COMPOSITE_MAX.sentinel_not_emitted_by_valid_paint_graph": (
            "FT_COLR_COMPOSITE_MAX sentinel parity needs a maintained valid-paint "
            "graph route proving the sentinel enum value is never emitted by "
            "pinned C for real composite paints"
        ),
    }
    if row.case_id in exact_cases:
        return exact_cases[row.case_id]
    mode_prefix = "ftcolor.FT_COLR_COMPOSITE_"
    if row.case_id.startswith(mode_prefix):
        mode_name = row.case_id[len(mode_prefix) :].split(".", 1)[0]
        if row.operation == "ftcolor.traverse_paint_graph":
            return (
                f"FT_COLR_COMPOSITE_{mode_name} traversal parity needs a "
                "maintained COLR paint graph route proving pinned C emits this "
                "mode at the same graph position without skipping nested source "
                "or backdrop paints"
            )
        if row.operation == "ftcolor.get_paint_graph":
            return (
                f"FT_COLR_COMPOSITE_{mode_name} graph parity needs a maintained "
                "COLR paint graph route proving pinned C exposes this composite "
                "mode with exact enum value and nested paint handles"
            )
        return (
            f"FT_COLR_COMPOSITE_{mode_name} parity needs a maintained COLR "
            "composite route proving exact pinned C payload, enum value, and "
            "nested paint behavior"
        )
    return None


def ftcolor_colorline_gradient_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific COLR colorline and gradient rows needing real routing."""
    if not row.operation.startswith("ftcolor."):
        return None
    exact_cases = {
        "ftcolor.FT_ColorLine.gradient_colorline_values": (
            "FT_ColorLine parity needs a maintained FT_Get_Paint route proving "
            "extend mode, stop count, and color stop iterator fields match "
            "pinned C public record output"
        ),
        "ftcolor.FT_ColorStop.iterator_output_values": (
            "FT_ColorStop parity needs a maintained FT_Get_Colorline_Stops "
            "route proving each emitted stop offset, color index, and alpha "
            "matches pinned C iteration output"
        ),
        "ftcolor.FT_ColorStopIterator.initialized_by_get_paint": (
            "FT_ColorStopIterator initialization parity needs a maintained "
            "FT_Get_Paint route proving iterator counters and opaque colorline "
            "state are initialized exactly like pinned C"
        ),
        "ftcolor.FT_ColorStopIterator.advanced_by_get_colorline_stops": (
            "FT_ColorStopIterator advance parity needs a maintained "
            "FT_Get_Colorline_Stops route proving iterator mutation and output "
            "preservation across successive calls match pinned C"
        ),
        "ftcolor.FT_Get_Colorline_Stops.success_iterates_static_colorline_stops": (
            "FT_Get_Colorline_Stops static-stop parity needs a maintained "
            "colorline route proving every static stop is emitted in pinned C "
            "order with exact scalar fields"
        ),
        "ftcolor.FT_Get_Colorline_Stops.success_iterates_variable_colorline_stops": (
            "FT_Get_Colorline_Stops variable-stop parity needs a maintained "
            "colorline route proving variation-adjusted stop values and "
            "iterator state match pinned C"
        ),
        "ftcolor.FT_Get_Colorline_Stops.end_of_iteration": (
            "FT_Get_Colorline_Stops end-of-iteration parity needs a maintained "
            "route proving false return and output preservation after the final "
            "stop match pinned C"
        ),
        "ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_values": (
            "FT_PaintLinearGradient parity needs a maintained route proving "
            "p0/p1/p2 vectors and attached colorline iterator match pinned C "
            "public union output"
        ),
        "ftcolor.FT_PaintRadialGradient.get_paint_radial_gradient_values": (
            "FT_PaintRadialGradient parity needs a maintained route proving "
            "circle centers, radii, and attached colorline iterator match "
            "pinned C public union output"
        ),
        "ftcolor.FT_PaintSweepGradient.get_paint_sweep_gradient_values": (
            "FT_PaintSweepGradient parity needs a maintained route proving "
            "center, start/end angles, and attached colorline iterator match "
            "pinned C public union output"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_LINEAR_GRADIENT.paint_linear_gradient_payload": (
            "FT_COLR_PAINTFORMAT_LINEAR_GRADIENT parity needs a maintained "
            "gradient route proving linear payload shape and nested colorline "
            "state match pinned C"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_RADIAL_GRADIENT.paint_radial_gradient_payload": (
            "FT_COLR_PAINTFORMAT_RADIAL_GRADIENT parity needs a maintained "
            "gradient route proving radial payload shape and nested colorline "
            "state match pinned C"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_SWEEP_GRADIENT.paint_sweep_gradient_payload": (
            "FT_COLR_PAINTFORMAT_SWEEP_GRADIENT parity needs a maintained "
            "gradient route proving sweep payload shape and nested colorline "
            "state match pinned C"
        ),
        "ftcolor.FT_PaintExtend.gradient_extend_runtime": (
            "FT_PaintExtend gradient parity needs a maintained traversal route "
            "proving public extend enum values emitted by gradients match "
            "pinned C"
        ),
    }
    if row.case_id in exact_cases:
        return exact_cases[row.case_id]
    extend_prefix = "ftcolor.FT_COLR_PAINT_EXTEND_"
    if row.case_id.startswith(extend_prefix):
        extend_name = row.case_id[len(extend_prefix) :].split(".", 1)[0]
        return (
            f"FT_COLR_PAINT_EXTEND_{extend_name} parity needs a maintained "
            "gradient colorline route proving pinned C emits this extend mode "
            "with exact enum value and preserves the same color stop iterator "
            "state"
        )
    return None


def ftcolor_root_paint_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific COLR root paint and root transform rows needing real routing."""
    if not row.operation.startswith("ftcolor."):
        return None
    variant = f" for variant {row.variant_id}" if row.variant_id else ""
    exact_cases = {
        "ftcolor.FT_Get_Color_Glyph_Paint.root_paint_success_no_root_transform": (
            "FT_Get_Color_Glyph_Paint no-root-transform parity needs a "
            "maintained root paint route proving initial opaque paint output "
            "and output preservation match pinned C when transforms are omitted"
        ),
        "ftcolor.FT_Get_Color_Glyph_Paint.root_paint_success_include_root_transform": (
            "FT_Get_Color_Glyph_Paint include-root-transform parity needs a "
            "maintained root paint route proving pinned C inserts or exposes "
            "the same root transform paint before downstream traversal"
        ),
        "ftcolor.FT_Get_Color_Glyph_Paint.downstream_paint_graph_contract": (
            "FT_Get_Color_Glyph_Paint downstream contract parity needs a "
            "maintained route proving opaque paint handles produced by root "
            "lookup can be consumed by FT_Get_Paint and graph traversal exactly "
            "like pinned C"
        ),
        "ftcolor.FT_Get_Paint.success_inserts_root_transform": (
            "FT_Get_Paint inserted-root-transform parity needs a maintained "
            "route proving the transform payload synthesized after root lookup "
            "matches pinned C public union output"
        ),
        "ftcolor.FT_Affine23.root_transform_values": (
            "FT_Affine23 root transform parity needs a maintained route proving "
            "xx, xy, dx, yx, yy, and dy fields match pinned C 16.16/26.6 values"
        ),
    }
    if row.case_id in exact_cases:
        return exact_cases[row.case_id]
    if row.case_id == "ftcolor.FT_COLOR_INCLUDE_ROOT_TRANSFORM.include_transform_runtime":
        return (
            "FT_COLOR_INCLUDE_ROOT_TRANSFORM parity needs a maintained root "
            f"paint route{variant} proving include flag behavior and resulting "
            "paint handle graph match pinned C"
        )
    if row.case_id == "ftcolor.FT_COLOR_NO_ROOT_TRANSFORM.omit_transform_runtime":
        return (
            "FT_COLOR_NO_ROOT_TRANSFORM parity needs a maintained root paint "
            f"route{variant} proving omitted-transform output and downstream "
            "paint graph match pinned C"
        )
    if row.case_id == "ftcolor.FT_Color_Root_Transform.root_transform_controls_initial_paint":
        return (
            "FT_Color_Root_Transform parity needs a maintained root paint "
            f"route{variant} proving enum control selects the same initial "
            "paint and transform insertion behavior as pinned C"
        )
    if row.case_id == "ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.included_root_transform_payload":
        return (
            "FT_COLR_PAINTFORMAT_TRANSFORM included-root payload parity needs "
            f"a maintained route{variant} proving transform paint format, "
            "affine fields, and nested paint handle match pinned C"
        )
    return None


def ftcolor_palette_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific COLR/CPAL palette rows needing real routing."""
    if not row.operation.startswith("ftcolor."):
        return None
    exact_cases = {
        "ftcolor.FT_Palette_Data_Get.error_color_layers_disabled": (
            "FT_Palette_Data_Get disabled-color-layers parity needs a "
            "maintained same-input route with a C-observable color font and "
            "active color-layer-disabled condition; the focused runtime "
            "currently has no font source for this error case"
        ),
        "ftcolor.FT_Palette_Select.error_color_layers_disabled": (
            "FT_Palette_Select disabled-color-layers parity needs a "
            "maintained same-input route with a C-observable color font and "
            "active color-layer-disabled condition; the focused runtime "
            "currently has no font source for this error case"
        ),
        "ftcolor.FT_Palette_Set_Foreground_Color.error_color_layers_disabled": (
            "FT_Palette_Set_Foreground_Color disabled-color-layers parity "
            "needs a maintained same-input route with a C-observable color "
            "font and active color-layer-disabled condition; the focused "
            "runtime currently has no font source for this error case"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcolor_paint_resolution_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific COLR FT_Get_Paint rows needing real paint-node routing."""
    if not row.operation.startswith("ftcolor."):
        return None
    exact_cases = {
        "ftcolor.FT_Get_Paint.success_resolves_each_supported_paint_format": (
            "FT_Get_Paint format-dispatch parity needs a maintained paint-node "
            "route proving every supported COLR paint format resolves to the "
            "same public union tag and payload as pinned C"
        ),
        "ftcolor.FT_OpaquePaint.produced_and_consumed_by_paint_apis": (
            "FT_OpaquePaint handle parity needs a maintained route proving root "
            "and nested opaque paint handles produced by public COLR APIs are "
            "consumed by FT_Get_Paint with the same lifetime and identity "
            "semantics as pinned C"
        ),
        "ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values": (
            "FT_PaintColrGlyph parity needs a maintained FT_Get_Paint route "
            "proving nested glyph ID and nested paint handle fields match "
            "pinned C public union output"
        ),
        "ftcolor.FT_PaintGlyph.get_paint_glyph_values": (
            "FT_PaintGlyph parity needs a maintained FT_Get_Paint route proving "
            "glyph ID and nested paint handle fields match pinned C public "
            "union output"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_COLR_GLYPH.paint_colr_glyph_runtime": (
            "FT_COLR_PAINTFORMAT_COLR_GLYPH parity needs a maintained route "
            "proving this format tag is emitted only with the pinned C "
            "FT_PaintColrGlyph payload shape"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_GLYPH.paint_glyph_payload": (
            "FT_COLR_PAINTFORMAT_GLYPH parity needs a maintained route proving "
            "this format tag is emitted with glyph ID plus nested paint handle "
            "payload matching pinned C"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_SOLID.paint_solid_color_index": (
            "FT_COLR_PAINTFORMAT_SOLID parity needs a maintained route proving "
            "solid paint color index, alpha, and palette-index semantics match "
            "pinned C"
        ),
        "ftcolor.FT_ColorIndex.solid_and_color_stop_values": (
            "FT_ColorIndex parity needs a maintained paint/colorline route "
            "proving solid paint and color-stop color indexes expose the same "
            "palette index, alpha, and foreground sentinel behavior as pinned C"
        ),
        "ftcolor.FT_PaintSolid.get_paint_solid_values": (
            "FT_PaintSolid parity needs a maintained FT_Get_Paint route proving "
            "color index and alpha fields match pinned C public union output"
        ),
        "ftcolor.FT_PaintFormat.paint_union_shape_runtime": (
            "FT_PaintFormat union-shape parity needs a maintained route proving "
            "public format tags select the same FT_COLR_Paint union arm and "
            "record layout as pinned C for each supported paint node"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcolor_transform_paint_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific COLR transform paint rows needing real affine routing."""
    if not row.operation.startswith("ftcolor."):
        return None
    exact_cases = {
        "ftcolor.FT_PaintRotate.get_paint_rotate_values": (
            "FT_PaintRotate parity needs a maintained FT_Get_Paint route "
            "proving angle, center coordinates, and nested paint handle fields "
            "match pinned C public union output"
        ),
        "ftcolor.FT_PaintScale.get_paint_scale_values": (
            "FT_PaintScale parity needs a maintained FT_Get_Paint route proving "
            "x/y scale factors, center coordinates, and nested paint handle "
            "fields match pinned C public union output"
        ),
        "ftcolor.FT_PaintSkew.get_paint_skew_values": (
            "FT_PaintSkew parity needs a maintained FT_Get_Paint route proving "
            "x/y skew angles, center coordinates, and nested paint handle "
            "fields match pinned C public union output"
        ),
        "ftcolor.FT_PaintTransform.get_paint_transform_values": (
            "FT_PaintTransform parity needs a maintained FT_Get_Paint route "
            "proving explicit affine matrix fields and nested paint handle "
            "match pinned C public union output"
        ),
        "ftcolor.FT_PaintTranslate.get_paint_translate_values": (
            "FT_PaintTranslate parity needs a maintained FT_Get_Paint route "
            "proving dx/dy translation values and nested paint handle fields "
            "match pinned C public union output"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_ROTATE.paint_rotate_normalized_payload": (
            "FT_COLR_PAINTFORMAT_ROTATE parity needs a maintained route proving "
            "FreeType-normalized rotate payload values and nested paint handle "
            "match pinned C"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_SCALE.paint_scale_normalized_payload": (
            "FT_COLR_PAINTFORMAT_SCALE parity needs a maintained route proving "
            "FreeType-normalized scale payload values, center handling, and "
            "nested paint handle match pinned C"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_SKEW.paint_skew_normalized_payload": (
            "FT_COLR_PAINTFORMAT_SKEW parity needs a maintained route proving "
            "FreeType-normalized skew payload values, center handling, and "
            "nested paint handle match pinned C"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.explicit_transform_payload": (
            "FT_COLR_PAINTFORMAT_TRANSFORM explicit payload parity needs a "
            "maintained route proving affine xx/xy/dx/yx/yy/dy fields and "
            "nested paint handle match pinned C"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_TRANSLATE.paint_translate_payload": (
            "FT_COLR_PAINTFORMAT_TRANSLATE parity needs a maintained route "
            "proving translation dx/dy payload values and nested paint handle "
            "match pinned C"
        ),
    }
    return exact_cases.get(row.case_id)


def ftcolor_clipbox_pending_reason(row: ConcreteInput) -> str | None:
    """Case- and variant-specific COLR clipbox pending rows."""
    if row.operation != "ftcolor.get_color_glyph_clipbox":
        return None
    runtime = runtime_id(row)
    variant = runtime.split("@", 1)[1] if "@" in runtime else "single"
    if row.case_id == "ftcolor.FT_ClipBox.color_glyph_clipbox_values":
        return (
            "FT_ClipBox public-record parity needs a maintained "
            "FT_Get_Color_Glyph_ClipBox route proving xMin/yMin/xMax/yMax "
            "fields are copied from pinned C clipbox output without wrapper "
            "normalization"
        )
    if (
        row.case_id
        == "ftcolor.FT_Get_Color_Glyph_ClipBox.clipbox_success_scaled_and_transformed"
    ):
        return (
            "FT_Get_Color_Glyph_ClipBox success parity needs a maintained "
            f"route for variant {variant} proving scaled size coordinates and "
            "active transform effects match pinned C output exactly"
        )
    if (
        row.case_id
        == "ftcolor.FT_Get_Color_Glyph_ClipBox.no_clipbox_returns_false_preserves_output"
    ):
        return (
            "FT_Get_Color_Glyph_ClipBox no-clipbox parity needs a maintained "
            "route proving a false return preserves the caller-provided output "
            "box bytes exactly like pinned C"
        )
    return None


def ftcolor_layer_iterator_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific COLR layer iterator rows needing real layer routing."""
    if not row.operation.startswith("ftcolor."):
        return None
    exact_cases = {
        "ftcolor.FT_Get_Color_Glyph_Layer.layer_iteration_success": (
            "FT_Get_Color_Glyph_Layer v0 parity needs a maintained layer route "
            "proving successive COLR v0 layer glyph indexes, color indexes, and "
            "iterator advancement match pinned C"
        ),
        "ftcolor.FT_Get_Color_Glyph_Layer.foreground_color_index": (
            "FT_Get_Color_Glyph_Layer foreground parity needs a maintained "
            "route proving foreground color index sentinel values are emitted "
            "and preserved exactly like pinned C"
        ),
        "ftcolor.FT_Get_Color_Glyph_Layer.terminal_false_preserves_last_outputs": (
            "FT_Get_Color_Glyph_Layer terminal parity needs a maintained route "
            "proving false return after the final layer preserves prior output "
            "fields and iterator state exactly like pinned C"
        ),
        "ftcolor.FT_Get_Paint_Layers.success_iterates_colr_v1_layers": (
            "FT_Get_Paint_Layers v1 parity needs a maintained layer route "
            "proving COLR v1 layer paint handles, layer count, and iterator "
            "advancement match pinned C"
        ),
        "ftcolor.FT_Get_Paint_Layers.end_of_iteration": (
            "FT_Get_Paint_Layers terminal parity needs a maintained route "
            "proving false return after the final v1 paint layer preserves "
            "output paint and iterator fields like pinned C"
        ),
        "ftcolor.FT_COLR_PAINTFORMAT_COLR_LAYERS.paint_colr_layers_payload": (
            "FT_COLR_PAINTFORMAT_COLR_LAYERS parity needs a maintained "
            "FT_Get_Paint route proving COLR_LAYERS payload initializes the "
            "same layer iterator fields and nested state as pinned C"
        ),
        "ftcolor.FT_PaintColrLayers.get_paint_initializes_layer_iterator": (
            "FT_PaintColrLayers parity needs a maintained FT_Get_Paint route "
            "proving layer count and initialized FT_LayerIterator output match "
            "pinned C public union output"
        ),
    }
    if row.case_id in {
        "ftcolor.FT_Get_Color_Glyph_Layer.layer_iteration_success",
        "ftcolor.FT_Get_Color_Glyph_Layer.foreground_color_index",
        "ftcolor.FT_Get_Color_Glyph_Layer.terminal_false_preserves_last_outputs",
    }:
        return None
    return exact_cases.get(row.case_id)


def ftcolor_colrv1_composite_real_parity_reason(row: ConcreteInput) -> str | None:
    if not row.operation.startswith("ftcolor."):
        return None
    if row.operation == "ftcolor.get_color_glyph_clipbox" and row.case_id in {
        "ftcolor.FT_ClipBox.color_glyph_clipbox_values",
        "ftcolor.FT_Get_Color_Glyph_ClipBox.clipbox_success_scaled_and_transformed",
        "ftcolor.FT_Get_Color_Glyph_ClipBox.no_clipbox_returns_false_preserves_output",
    }:
        return (
            "FT_Get_Color_Glyph_ClipBox validates ClipList format 1 scaling, "
            "active face transform application, public FT_ClipBox field "
            "copying, and false-return output preservation through the "
            "maintained COLRv1 clipbox fixture, pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI"
        )
    # These rows are routed by `emit_colr_all_paints_case` and the matching
    # Rust/C-ABI/WASM all-paints harness path.  Their fixture JSON still
    # declares a future malformed-font asset, but this maintained public
    # payload route consumes only `colr-v1-all-paints.ttf` and compares the
    # public union/iterator output against pinned C.
    if row.case_id in {
        "ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values",
        "ftcolor.FT_PaintColrLayers.get_paint_initializes_layer_iterator",
    }:
        return (
            "COLRv1 all-paints FT_Get_Paint route validates PaintColrGlyph "
            "payloads and initialized PaintColrLayers iterators through pinned "
            "C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if unresolved_assets_reason(row) is not None:
        return None
    if row.case_id == "ftcolor.FT_Get_Color_Glyph_Paint.root_paint_success_no_root_transform":
        return (
            "FT_Get_Color_Glyph_Paint no-root-transform root lookup validates "
            "through the maintained COLRv1 composite fixture, pinned C oracle, "
            "Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id == "ftcolor.FT_Get_Color_Glyph_Paint.root_paint_success_include_root_transform":
        return (
            "FT_Get_Color_Glyph_Paint include-root-transform lookup validates "
            "the inserted transform paint through the maintained COLRv1 root "
            "transform fixture, pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_Get_Color_Glyph_Paint.downstream_paint_graph_contract",
        "ftcolor.FT_OpaquePaint.produced_and_consumed_by_paint_apis",
    }:
        return (
            "COLRv1 opaque paint production and downstream FT_Get_Paint "
            "consumption validate for the maintained solid/glyph/composite "
            "paint graph through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_COLR_PAINTFORMAT_SOLID.paint_solid_color_index",
        "ftcolor.FT_COLR_PAINTFORMAT_GLYPH.paint_glyph_payload",
        "ftcolor.FT_COLR_PAINTFORMAT_COMPOSITE.paint_composite_payload",
    }:
        return (
            "COLRv1 FT_COLR_Paint format dispatch validates the maintained "
            "solid, glyph, and composite payload rows through pinned C oracle, "
            "Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_PaintSolid.get_paint_solid_values",
        "ftcolor.FT_PaintGlyph.get_paint_glyph_values",
        "ftcolor.FT_PaintComposite.get_paint_composite_values",
    }:
        return (
            "COLRv1 FT_Get_Paint public union payload values validate for "
            "PaintSolid, PaintGlyph, and PaintComposite through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_Get_Paint.success_resolves_each_supported_paint_format",
        "ftcolor.FT_Get_Paint.success_inserts_root_transform",
        "ftcolor.FT_Affine23.root_transform_values",
        "ftcolor.FT_ColorStopIterator.initialized_by_get_paint",
        "ftcolor.FT_ColorIndex.solid_and_color_stop_values",
        "ftcolor.FT_PaintFormat.paint_union_shape_runtime",
        "ftcolor.FT_PaintColrGlyph.get_paint_colr_glyph_values",
        "ftcolor.FT_PaintColrLayers.get_paint_initializes_layer_iterator",
    }:
        return (
            "COLRv1 all-paints FT_Get_Paint route validates supported paint "
            "formats, initialized colorline and layer iterators, PaintColrGlyph "
            "payloads, and inserted root-transform affine values through pinned "
            "C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_Composite_Mode.paint_composite_modes_runtime",
        "ftcolor.FT_COLR_COMPOSITE_MAX.sentinel_not_emitted_by_valid_paint_graph",
    }:
        return (
            "COLRv1 PaintComposite mode graph validates every real composite "
            "mode and sentinel absence through pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI"
        )
    if row.case_id == "ftcolor.FT_COLR_PAINTFORMAT_COLR_LAYERS.paint_colr_layers_payload":
        return (
            "COLRv1 PaintColrLayers payload and FT_Get_Paint_Layers traversal "
            "validate through the maintained layer-list fixture, pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_Get_Paint_Layers.success_iterates_colr_v1_layers",
        "ftcolor.FT_Get_Paint_Layers.end_of_iteration",
        "ftcolor.FT_LayerIterator.initialized_and_advanced_by_paint_layers_v1",
    }:
        return (
            "COLRv1 FT_Get_Paint_Layers success and terminal iteration "
            "validate layer paint handles, iterator fields, and exhausted-call "
            "preservation through the maintained layer-list fixture, pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id == "ftcolor.FT_COLR_PAINTFORMAT_COLR_GLYPH.paint_colr_glyph_runtime":
        return (
            "COLRv1 PaintColrGlyph payload and referenced BaseGlyphV1List root "
            "lookup validate through the maintained recursive COLRv1 fixture, "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_COLR_PAINTFORMAT_ROTATE.paint_rotate_normalized_payload",
        "ftcolor.FT_COLR_PAINTFORMAT_SCALE.paint_scale_normalized_payload",
        "ftcolor.FT_COLR_PAINTFORMAT_SKEW.paint_skew_normalized_payload",
        "ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.explicit_transform_payload",
        "ftcolor.FT_COLR_PAINTFORMAT_TRANSLATE.paint_translate_payload",
    }:
        return (
            "COLRv1 transform paint format dispatch and FreeType-normalized "
            "public payload values validate through the maintained transform "
            "fixture, pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_PaintRotate.get_paint_rotate_values",
        "ftcolor.FT_PaintScale.get_paint_scale_values",
        "ftcolor.FT_PaintSkew.get_paint_skew_values",
        "ftcolor.FT_PaintTransform.get_paint_transform_values",
        "ftcolor.FT_PaintTranslate.get_paint_translate_values",
    }:
        return (
            "COLRv1 FT_Get_Paint public transform union records validate "
            "exact child opaque-paint classes and fixed-point fields through "
            "the maintained transform fixture, pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_COLR_PAINTFORMAT_LINEAR_GRADIENT.paint_linear_gradient_payload",
        "ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_static_values",
        "ftcolor.FT_COLR_PAINTFORMAT_RADIAL_GRADIENT.paint_radial_gradient_payload",
        "ftcolor.FT_COLR_PAINTFORMAT_SWEEP_GRADIENT.paint_sweep_gradient_payload",
        "ftcolor.FT_PaintRadialGradient.get_paint_radial_gradient_values",
        "ftcolor.FT_PaintSweepGradient.get_paint_sweep_gradient_values",
        "ftcolor.FT_COLR_PAINT_EXTEND_PAD.colorline_extend_pad",
        "ftcolor.FT_COLR_PAINT_EXTEND_REPEAT.colorline_extend_repeat",
        "ftcolor.FT_COLR_PAINT_EXTEND_REFLECT.colorline_extend_reflect",
        "ftcolor.FT_PaintExtend.gradient_extend_runtime",
        "ftcolor.FT_ColorLine.gradient_colorline_values",
        "ftcolor.FT_Get_Colorline_Stops.success_iterates_static_colorline_stops",
        "ftcolor.FT_Get_Colorline_Stops.end_of_iteration",
        "ftcolor.FT_ColorStopIterator.advanced_by_get_colorline_stops",
    }:
        return (
            "COLRv1 static gradient payloads and ColorLine stop iteration "
            "validate through the maintained PAD/REPEAT/REFLECT static "
            "gradient fixture, pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_ColorStop.iterator_output_values",
        "ftcolor.FT_PaintLinearGradient.get_paint_linear_gradient_variable_values",
        "ftcolor.FT_Get_Colorline_Stops.success_iterates_variable_colorline_stops",
    }:
        return (
            "COLRv1 variable gradient PaintLinearGradient payload and "
            "VarColorStop iteration validate default and non-default wght/GRAD "
            "design-coordinate runs through the maintained variable-gradient "
            "fixture, pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftcolor.FT_COLOR_INCLUDE_ROOT_TRANSFORM.include_transform_runtime",
        "ftcolor.FT_COLOR_NO_ROOT_TRANSFORM.omit_transform_runtime",
        "ftcolor.FT_Color_Root_Transform.root_transform_controls_initial_paint",
        "ftcolor.FT_COLR_PAINTFORMAT_TRANSFORM.included_root_transform_payload",
    }:
        return (
            "COLRv1 included-root-transform synthesis validates exact public "
            "PaintTransform affine fields and nested opaque paint class through "
            "the maintained root-transform fixture, pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI"
        )
    if row.case_id.startswith("ftcolor.FT_COLR_COMPOSITE_") and (
        row.case_id.endswith(".paint_composite_runtime")
        or row.case_id.endswith(".paint_composite_mode_runtime")
    ):
        return (
            "COLRv1 PaintComposite node validates exact composite_mode and "
            "source/backdrop paint traversal through pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI"
        )
    return None


def ftcolor_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for the COLR/CPAL subsystem that do not have a maintained success route."""
    if not row.operation.startswith("ftcolor."):
        return None
    if ftcolor_colrv1_composite_real_parity_reason(row):
        return None
    if absent_or_noop_surface_real_parity_reason(row):
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    composite_pending = ftcolor_composite_pending_reason(row)
    if composite_pending:
        return composite_pending
    colorline_gradient_pending = ftcolor_colorline_gradient_pending_reason(row)
    if colorline_gradient_pending:
        return colorline_gradient_pending
    root_paint_pending = ftcolor_root_paint_pending_reason(row)
    if root_paint_pending:
        return root_paint_pending
    palette_pending = ftcolor_palette_pending_reason(row)
    if palette_pending:
        return palette_pending
    paint_resolution_pending = ftcolor_paint_resolution_pending_reason(row)
    if paint_resolution_pending:
        return paint_resolution_pending
    transform_paint_pending = ftcolor_transform_paint_pending_reason(row)
    if transform_paint_pending:
        return transform_paint_pending
    clipbox_pending = ftcolor_clipbox_pending_reason(row)
    if clipbox_pending:
        return clipbox_pending
    layer_iterator_pending = ftcolor_layer_iterator_pending_reason(row)
    if layer_iterator_pending:
        return layer_iterator_pending
    return None


def t1tables_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for Type1 table runtime data that do not have a maintained route."""
    if not row.operation.startswith("t1tables."):
        return None
    if operation_is_compile_contract(row.operation):
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    pending_cases = {
        "t1tables.FT_Get_PS_Font_Info.signature_and_behavior_matrix": (
            "FT_Get_PS_Font_Info parity needs a C-openable Type1/CFF fixture "
            "and exact T1_FontInfo string/scalar record comparison across "
            "pinned C, Rust FFI, C ABI, and WASM ABI"
        ),
        "t1tables.FT_Get_PS_Font_Private.signature_and_behavior_matrix": (
            "FT_Get_PS_Font_Private parity needs a C-openable Type1/CFF "
            "fixture and exact T1_Private dictionary scalar/array comparison "
            "across all ABI lanes"
        ),
        "t1tables.FT_Has_PS_Glyph_Names.signature_and_behavior_matrix": (
            "FT_Has_PS_Glyph_Names parity needs Type1/CFF fixtures proving "
            "pinned C glyph-name availability and matching Rust/C-ABI/WASM "
            "boolean output"
        ),
        "t1tables.T1_BLEND_BLUE_SCALE.private_blue_scale_runtime_value": (
            "T1_BLEND_BLUE_SCALE parity needs a Multiple Master Type1 fixture "
            "and exact private blue_scale blend value comparison"
        ),
        "t1tables.T1_BLEND_BLUE_SHIFT.private_blue_shift_runtime_value": (
            "T1_BLEND_BLUE_SHIFT parity needs a Multiple Master Type1 fixture "
            "and exact private blue_shift blend value comparison"
        ),
        "t1tables.T1_BLEND_BLUE_VALUES.private_blue_values_runtime_array": (
            "T1_BLEND_BLUE_VALUES parity needs exact private blue_values array "
            "length and element comparison from a Multiple Master Type1 fixture"
        ),
        "t1tables.T1_BLEND_FAMILY_BLUES.private_family_blues_runtime_array": (
            "T1_BLEND_FAMILY_BLUES parity needs exact private family_blues "
            "array length and element comparison"
        ),
        "t1tables.T1_BLEND_FAMILY_OTHER_BLUES.private_family_other_blues_runtime_array": (
            "T1_BLEND_FAMILY_OTHER_BLUES parity needs exact private "
            "family_other_blues array length and element comparison"
        ),
        "t1tables.T1_BLEND_FORCE_BOLD.private_force_bold_runtime_value": (
            "T1_BLEND_FORCE_BOLD parity needs exact private force_bold blend "
            "value comparison and C boolean normalization"
        ),
        "t1tables.T1_BLEND_ITALIC_ANGLE.font_info_italic_angle_runtime_value": (
            "T1_BLEND_ITALIC_ANGLE parity needs exact font-info italic_angle "
            "blend value comparison from a Multiple Master Type1 fixture"
        ),
        "t1tables.T1_BLEND_OTHER_BLUES.private_other_blues_runtime_array": (
            "T1_BLEND_OTHER_BLUES parity needs exact private other_blues array "
            "length and element comparison"
        ),
        "t1tables.T1_BLEND_STANDARD_HEIGHT.private_standard_height_runtime_value": (
            "T1_BLEND_STANDARD_HEIGHT parity needs exact private standard_height "
            "blend value comparison"
        ),
        "t1tables.T1_BLEND_STANDARD_WIDTH.private_standard_width_runtime_value": (
            "T1_BLEND_STANDARD_WIDTH parity needs exact private standard_width "
            "blend value comparison"
        ),
        "t1tables.T1_BLEND_STEM_SNAP_HEIGHTS.private_snap_heights_runtime_array": (
            "T1_BLEND_STEM_SNAP_HEIGHTS parity needs exact private snap_heights "
            "array length and element comparison"
        ),
        "t1tables.T1_BLEND_STEM_SNAP_WIDTHS.private_snap_widths_runtime_array": (
            "T1_BLEND_STEM_SNAP_WIDTHS parity needs exact private snap_widths "
            "array length and element comparison"
        ),
        "t1tables.T1_BLEND_UNDERLINE_POSITION.blend_dictionary_runtime": (
            "T1_BLEND_UNDERLINE_POSITION parity needs maintained blend "
            "dictionary routing and exact underline_position output comparison"
        ),
        "t1tables.T1_BLEND_UNDERLINE_THICKNESS.blend_dictionary_runtime": (
            "T1_BLEND_UNDERLINE_THICKNESS parity needs maintained blend "
            "dictionary routing and exact underline_thickness output comparison"
        ),
        "t1tables.T1_Blend_Flags.font_info_blend_group": (
            "T1_Blend_Flags font-info group parity needs runtime proof that "
            "public blend flags select the same font-info dictionary fields as "
            "pinned C"
        ),
        "t1tables.T1_Blend_Flags.private_blend_group": (
            "T1_Blend_Flags private group parity needs runtime proof that "
            "public blend flags select the same private dictionary fields as "
            "pinned C"
        ),
        "t1tables.T1_ENCODING_TYPE_ARRAY.ps_font_value_encoding_type": (
            "T1_ENCODING_TYPE_ARRAY parity needs a Type1 encoding fixture and "
            "exact FT_Get_PS_Font_Value encoding-type output comparison"
        ),
        "t1tables.T1_ENCODING_TYPE_EXPERT.ps_font_value_encoding_type": (
            "T1_ENCODING_TYPE_EXPERT parity needs an expert-encoding fixture "
            "and exact FT_Get_PS_Font_Value encoding-type output comparison"
        ),
        "t1tables.T1_ENCODING_TYPE_ISOLATIN1.ps_font_value_encoding_type": (
            "T1_ENCODING_TYPE_ISOLATIN1 parity needs an ISO Latin-1 encoding "
            "fixture and exact FT_Get_PS_Font_Value output comparison"
        ),
        "t1tables.T1_ENCODING_TYPE_NONE.ps_font_value_encoding_type": (
            "T1_ENCODING_TYPE_NONE parity needs a no-encoding Type1 fixture and "
            "exact FT_Get_PS_Font_Value output comparison"
        ),
        "t1tables.T1_ENCODING_TYPE_STANDARD.ps_font_value_encoding_type": (
            "T1_ENCODING_TYPE_STANDARD parity needs a standard-encoding Type1 "
            "fixture and exact FT_Get_PS_Font_Value output comparison"
        ),
        "t1tables.T1_EncodingType.array_encoding_runtime_case": (
            "T1_EncodingType array parity needs maintained encoding extraction "
            "that compares array encoding records and glyph-name resolution"
        ),
        "t1tables.T1_EncodingType.standard_or_expert_runtime_cases": (
            "T1_EncodingType standard/expert parity needs maintained fixtures "
            "that prove standard and expert encoding classification separately"
        ),
    }
    return pending_cases.get(row.case_id)


def ftgxval_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for GX/classic kern validation data that do not have a maintained route."""
    if not row.operation.startswith("ftgxval."):
        return None
    if absent_or_noop_surface_real_parity_reason(row):
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    pending_cases = {
        "ftgxval.FT_ClassicKern_Free.frees_classic_kern_validation_buffer": (
            "FT_ClassicKern_Free parity needs a maintained validate-then-free "
            "route proving C allocation ownership and free semantics for "
            "non-null classic-kern validation buffers across Rust FFI, C ABI, "
            "and WASM ABI; generic null/no-op free is not enough"
        ),
        "ftgxval.FT_ClassicKern_Validate.validates_ms_classic_kern": (
            "FT_ClassicKern_Validate MS classic-kern parity needs a C-openable "
            "fixture with Microsoft classic kern data and exact validation "
            "output pointer/length bytes plus error code across all ABI lanes"
        ),
        "ftgxval.FT_ClassicKern_Validate.validates_apple_classic_kern": (
            "FT_ClassicKern_Validate Apple classic-kern parity needs a "
            "C-openable Apple classic kern fixture and exact validation "
            "buffer/error/lifetime comparison; MS-kern success does not prove "
            "Apple selector behavior"
        ),
        "ftgxval.FT_TrueTypeGX_Free.frees_gx_validation_buffer": (
            "FT_TrueTypeGX_Free parity needs a maintained validate-then-free "
            "route proving ownership/free semantics for table buffers returned "
            "by FT_TrueTypeGX_Validate across all ABI lanes"
        ),
        "ftgxval.FT_TrueTypeGX_Validate.validates_selected_gx_tables": (
            "FT_TrueTypeGX_Validate selected-table parity needs a C-openable "
            "GX/AAT fixture and exact output slots for selected tables across "
            "pinned C, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftgxval.FT_TrueTypeGX_Validate.validates_all_gx_tables": (
            "FT_TrueTypeGX_Validate all-table parity needs a C-openable GX/AAT "
            "fixture and exact output slots/errors/lifetimes for every "
            "requested table; selected subset success is not enough"
        ),
        "ftgxval.FT_TrueTypeGX_Validate.respects_table_length": (
            "FT_TrueTypeGX_Validate length parity needs malformed/truncated GX "
            "tables proving pinned C length validation and exact error/output "
            "pointer handling"
        ),
        "ftgxval.FT_VALIDATE_APPLE.runtime_selects_apple_classic_kern": (
            "FT_VALIDATE_APPLE runtime parity needs a classic-kern route proving "
            "the selector chooses Apple kern validation/output rather than MS "
            "behavior"
        ),
        "ftgxval.FT_VALIDATE_CKERN.runtime_accepts_ms_or_apple": (
            "FT_VALIDATE_CKERN parity needs a maintained classic-kern route "
            "proving the selector accepts the correct MS/Apple classic kern "
            "variant and returns exact buffer/error output"
        ),
        "ftgxval.FT_VALIDATE_CKERN.output_table_lifetime": (
            "FT_VALIDATE_CKERN lifetime parity needs a validate/free route "
            "proving returned table buffers remain valid until "
            "FT_ClassicKern_Free and are freed exactly once"
        ),
        "ftgxval.FT_VALIDATE_opbd.gx_validate_selects_opbd_table": (
            "FT_VALIDATE_opbd parity needs a GX/AAT fixture with an opbd table "
            "and exact selected-table output slot/error comparison"
        ),
        "ftgxval.FT_VALIDATE_opbd_INDEX.indexes_gx_validate_output_slot": (
            "FT_VALIDATE_opbd_INDEX parity needs maintained proof that the "
            "public index maps to the same FT_TrueTypeGX_Validate output slot "
            "as pinned C"
        ),
        "ftgxval.FT_VALIDATE_prop.gx_validate_selects_prop_table": (
            "FT_VALIDATE_prop parity needs a GX/AAT fixture with a prop table "
            "and exact selected-table output slot/error comparison"
        ),
        "ftgxval.FT_VALIDATE_prop_INDEX.indexes_gx_validate_output_slot": (
            "FT_VALIDATE_prop_INDEX parity needs maintained proof that the "
            "public index maps to the same FT_TrueTypeGX_Validate output slot "
            "as pinned C"
        ),
        "ftgxval.FT_VALIDATE_trak.gx_validate_selects_trak_table": (
            "FT_VALIDATE_trak parity needs a GX/AAT fixture with a trak table "
            "and exact selected-table output slot/error comparison"
        ),
        "ftgxval.FT_VALIDATE_trak_INDEX.indexes_gx_validate_output_slot": (
            "FT_VALIDATE_trak_INDEX parity needs maintained proof that the "
            "public index maps to the same FT_TrueTypeGX_Validate output slot "
            "as pinned C"
        ),
    }
    return pending_cases.get(row.case_id)


def absent_or_noop_surface_real_parity_reason(row: ConcreteInput) -> str | None:
    """Contained no-op/absent-table public rows with maintained same-input routes."""
    color_cases = {
        "ftcolor.FT_Palette_Data_Get.success_sfnt_without_cpal",
    }
    if row.case_id in color_cases:
        return (
            "FT color palette SFNT-without-CPAL behavior validates through pinned "
            "C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.case_id in {
        "ftgxval.FT_TrueTypeGX_Free.null_face_noop",
        "ftgxval.FT_ClassicKern_Free.null_face_noop",
    }:
        return (
            "GX/classic-kern validation free null-face no-op validates through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    return None


def header_or_layout_compile_contract_reason(row: ConcreteInput) -> str | None:
    """Rows whose manifest expectations are public header, macro, or layout contracts."""
    compile_contract_cases = {
        "ftglyph.FT_Glyph_BBox_Mode.enum_variants_match_header",
        "ftglyph.FT_Glyph_BBox_Mode.deprecated_lowercase_aliases_match",
        "ftimage.FT_IMAGE_TAG.override_contract_matches_c",
        "ftcid.FT_Get_CID_Registry_Ordering_Supplement.public_header_signature",
        "ftwinfnt.FT_WinFNT_ID_DEFAULT.invalid_as_real_font_charset",
        "ftmm.T1_MAX_MM_AXIS.record_array_capacity",
        "ftmm.T1_MAX_MM_MAP_POINTS.axis_map_capacity",
        "t1tables.T1_BLEND_MAX.sentinel_not_runtime_field",
    }
    if row.case_id in compile_contract_cases:
        return (
            "manifest expectation is a public header, macro, enum, or layout "
            "contract verified by the ABI audit rather than runtime subsystem behavior"
        )
    return None


def ftmm_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for MM/variation runtime data that do not have a maintained route."""
    ftmm_rows_without_maintained_route = {
        "ftmm.FT_Done_MM_Var.import_contract": (
            "FT_Done_MM_Var import contract is a compile/layout obligation; "
            "runtime descriptor ownership must stay routed through explicit "
            "FT_Get_MM_Var then FT_Done_MM_Var rows"
        ),
        "ftmm.FT_Get_Var_Design_Coordinates.excess_output_coordinates_zero_filled": (
            "FT_Get_Var_Design_Coordinates excess-output fixture is not a "
            "sound same-input parity row for the current TrueType variable "
            "font: pinned FreeType 2.14.3 `TT_Get_Var_Design` clamps the active "
            "axis count but then reads default values past the axis array for "
            "excess outputs, while Type1 MM zero-fills; promoting Rust's safe "
            "zero-fill or modeling pinned-C adjacent memory would be a green "
            "placeholder"
        ),
        "ftmm.FT_Set_MM_Blend_Coordinates.output_changes_for_active_blend": (
            "FT_Set_MM_Blend_Coordinates output parity needs a maintained MM "
            "fixture whose pinned C route is a glyph-output success row: the "
            "current TrueType variable fixture gvar-hvar-wght.ttf returns "
            "error -2 from FT_Set_MM_Blend_Coordinates for coords=[65536], and "
            "reusing the Var-blend success path would be a green placeholder"
        ),
        "ftmm.FT_Set_MM_Design_Coordinates.output_changes_for_mm_design": (
            "FT_Set_MM_Design_Coordinates Adobe-MM glyph-output fixture is not "
            "currently a C-observable success row: pinned FreeType 2.14.3 "
            "returns OK for FT_Set_Pixel_Sizes and FT_Set_MM_Design_Coordinates "
            "on the maintained Type1 MM fixture, then FT_Load_Glyph returns "
            "error 6 for the declared glyph_index=42; promoting this as glyph "
            "output parity would be a green placeholder until the input names a "
            "glyph that C can load successfully"
        ),
        "ftmm.T1_MAX_MM_AXIS.record_array_capacity": (
            "T1_MAX_MM_AXIS runtime parity needs a maintained Type1 MM "
            "descriptor route proving public arrays are capped and populated "
            "with pinned-C capacity semantics"
        ),
        "ftmm.T1_MAX_MM_MAP_POINTS.axis_map_capacity": (
            "T1_MAX_MM_MAP_POINTS runtime parity needs a maintained Type1 MM "
            "axis-map route proving blend design maps are capped and exposed "
            "with pinned-C capacity semantics"
        ),
    }
    if row.case_id not in ftmm_rows_without_maintained_route:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    return ftmm_rows_without_maintained_route[row.case_id]


def ftmodapi_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for module/library lifecycle data that do not have a maintained route."""
    if row.case_id == "ftmodapi.FT_Add_Module.add_minimal_module_success":
        if (
            row.operation == "ftmodapi.add_module"
            and row.params.get("library") == "new_from_FT_New_Library_without_modules"
            and isinstance(row.params.get("module_class"), dict)
            and row.params["module_class"].get("module_name") == "fixture_minimal"
            and row.params["module_class"].get("module_init") == "record_call_then_ok"
        ):
            return None
        return (
            "FT_Add_Module success requires a maintained synthetic module-class "
            "route matching freetype/src/base/ftobjs.c:5058-5168: version/name "
            "checks, allocation, module->library/memory initialization, optional "
            "renderer/hinter/driver side effects, module_init callback, module "
            "table insertion, and FT_Get_Module lookup across pinned C, Rust "
            "FFI, C ABI, and WASM; null/future-version/duplicate error rows "
            "do not prove success installation"
        )
    if row.case_id == "ftmodapi.FT_Done_Library.final_destroy_closes_faces_and_modules":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Done_Library final-destroy success requires a maintained "
            "library lifecycle route matching freetype/src/base/ftobjs.c:5542-"
            "5620: refcount reaches zero, driver-owned faces are closed in C "
            "order, modules are removed in reverse table order, destructors run, "
            "and the library becomes unusable across Rust FFI, C ABI, and WASM; "
            "null-library and non-final refcount rows are not final destruction"
        )
    if row.case_id == "ftmodapi.FT_MODULE_RENDERER.renderer_module_registration":
        if (
            row.operation == "ftmodapi.add_module"
            and row.params.get("library") == "new_from_FT_New_Library_then_FT_Add_Default_Modules"
            and isinstance(row.params.get("module_class"), dict)
            and row.params["module_class"].get("module_name") == "fixture_renderer"
            and "FT_MODULE_RENDERER" in row.params["module_class"].get("module_flags", [])
            and row.params["module_class"].get("module_interface")
            == "synthetic_renderer_interface"
            and row.params["module_class"].get("module_init") == "record_call_then_ok"
        ):
            return None
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_MODULE_RENDERER registration requires a maintained renderer "
            "module-class route through FT_Add_Module that proves ft_add_renderer "
            "runs before module_init, updates the renderer list/current renderer, "
            "cleans raster state on init failure, and preserves lookup/removal "
            "behavior across pinned C, Rust FFI, C ABI, and WASM; the header "
            "constant value alone would be a green placeholder"
        )
    if row.case_id == "ftmodapi.FT_MODULE_STYLER.styler_module_registration":
        if (
            row.operation == "ftmodapi.add_module"
            and row.params.get("library") == "new_from_FT_New_Library_then_FT_Add_Default_Modules"
            and isinstance(row.params.get("module_class"), dict)
            and row.params["module_class"].get("module_name") == "fixture_styler"
            and "FT_MODULE_STYLER" in row.params["module_class"].get("module_flags", [])
            and row.params["module_class"].get("module_interface") == "fixture_private_interface"
            and row.params["module_class"].get("module_init") == "record_call_then_ok"
        ):
            return None
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_MODULE_STYLER registration requires a maintained synthetic "
            "module-class route proving the styler flag is stored in the module "
            "class and does not trigger renderer, hinter, or driver side effects "
            "while remaining observable through module lookup across pinned C, "
            "Rust FFI, C ABI, and WASM; constant-value parity is not lifecycle "
            "parity"
        )
    if row.case_id == "ftmodapi.FT_Module_Class.fields_drive_module_lifecycle":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Module_Class lifecycle parity requires a maintained class-field "
            "facade that exercises module_name/version/requires/flags/size, "
            "module_interface, module_init, and module_done through "
            "FT_Add_Module, FT_Get_Module_Interface, FT_Remove_Module, and "
            "FT_Done_Library across pinned C, Rust FFI, C ABI, and WASM; layout "
            "or import-contract checks alone do not prove field-driven behavior"
        )
    if row.case_id == "ftmodapi.FT_Module_Interface.requester_return_type":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Module_Interface requester behavior requires a maintained "
            "module_interface route matching FT_Get_Module_Interface in "
            "freetype/src/base/ftobjs.c:5199-5207: look up the named module and "
            "return clazz->module_interface exactly, including null library/name "
            "and missing-module cases, across Rust FFI, C ABI, and WASM; generic "
            "module lookup does not prove the opaque interface payload"
        )
    if row.case_id == "ftmodapi.FT_Remove_Module.removes_installed_module":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Remove_Module success requires a maintained add-get-remove route "
            "matching freetype/src/base/ftobjs.c:5261-5298: find the exact "
            "module pointer in the table, compact following entries, null the "
            "tail slot, call Destroy_Module/module_done, and prove later lookup "
            "failure across pinned C, Rust FFI, C ABI, and WASM; null or foreign "
            "module error rows are not removal success"
        )
    return None


def ftdriver_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for driver/autohinter properties that do not have a maintained route."""
    ftdriver_rows_without_maintained_route = {
        "ftdriver.FT_CFF_HINTING_ADOBE.hinting_engine_property_runtime": (
            "CFF Adobe hinting-engine runtime parity needs maintained "
            "FT_Property_Set/Get routing for the CFF driver and a C-openable "
            "CFF fixture where the selected engine's public metrics, outline, "
            "or bitmap behavior is compared across all ABI lanes"
        ),
        "ftdriver.FT_CFF_HINTING_FREETYPE.hinting_engine_property_runtime": (
            "CFF FreeType hinting-engine runtime parity needs maintained "
            "FT_Property_Set/Get routing for the CFF driver and the same "
            "C-openable CFF output comparison; scalar macro values alone do "
            "not prove runtime behavior"
        ),
        "ftdriver.FT_HINTING_ADOBE.hinting_engine_property_runtime": (
            "TrueType Adobe hinting-engine runtime parity needs maintained "
            "driver property routing and a bytecode-sensitive TrueType fixture "
            "that compares public hinted output across pinned C, Rust FFI, "
            "C ABI, and WASM ABI"
        ),
        "ftdriver.FT_HINTING_FREETYPE.hinting_engine_property_runtime": (
            "TrueType FreeType hinting-engine runtime parity needs maintained "
            "driver property routing and the matching bytecode-sensitive "
            "output comparison; accepting a property-set no-op would be a "
            "green placeholder"
        ),
    }
    reason = ftdriver_rows_without_maintained_route.get(row.case_id)
    if reason is None:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    return reason


def property_service_pending_reason(row: ConcreteInput) -> str | None:
    property_rows_without_maintained_route = {
        "ftdriver.FT_Prop_GlyphToScriptMap.map_mutation_affects_autohint_script": (
            "FT_Prop_GlyphToScriptMap mutation parity needs maintained typed "
            "property routing plus an autohint glyph-load observation proving "
            "map changes affect script selection like pinned C"
        ),
        "ftdriver.FT_Prop_IncreaseXHeight.limit_changes_autohint_x_height": (
            "FT_Prop_IncreaseXHeight glyph-output parity needs maintained "
            "typed property routing plus an autohint load proving the x-height "
            "adjustment changes metrics/outline output like pinned C"
        ),
    }
    if row.case_id not in property_rows_without_maintained_route:
        return None
    return property_rows_without_maintained_route[row.case_id]


def ftincrem_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for incremental font callbacks that do not have a maintained route."""
    ftincrem_rows_without_maintained_route = {
        "ftincrem.FT_Incremental.handle_passed_without_deref": (
            "FT_Incremental handle identity parity needs a maintained "
            "incremental-font route that passes the client object through "
            "FT_PARAM_TAG_INCREMENTAL and proves pinned C forwards the opaque "
            "handle to callbacks without dereferencing it, then compares Rust "
            "FFI, C ABI, and WASM ABI event logs"
        ),
        "ftincrem.FT_Incremental.lifetime_owned_by_client": (
            "FT_Incremental client lifetime parity needs a maintained route "
            "that proves FreeType stores only the client-owned interface/object "
            "for the face lifetime and does not free it; generic handle "
            "non-nullness would be a green placeholder"
        ),
        "ftincrem.FT_Incremental_FuncsRec.required_and_optional_callbacks": (
            "FT_Incremental_FuncsRec validation parity needs exact pinned C "
            "behavior for required get_glyph_data and get_glyph_metrics "
            "callbacks, optional free_glyph_data, null entries, and open/load "
            "error timing across Rust FFI, C ABI, and WASM ABI"
        ),
        "ftincrem.FT_Incremental_FuncsRec.glyph_data_success_and_release": (
            "FT_Incremental_FuncsRec glyph-data parity needs a maintained "
            "incremental glyph-load fixture that records get_glyph_data, "
            "glyph-byte ownership, release callback ordering, and public glyph "
            "output across pinned C, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftincrem.FT_Incremental_Interface.parameter_data_cast_shape": (
            "FT_Incremental_Interface parameter-cast parity needs a maintained "
            "FT_Open_Face FT_Parameter route proving the data pointer is "
            "interpreted as FT_Incremental_InterfaceRec* with exact null/bad "
            "shape handling across all ABI lanes"
        ),
        "ftincrem.FT_Incremental_InterfaceRec.open_face_stores_interface": (
            "FT_Incremental_InterfaceRec storage parity needs a maintained "
            "FT_Open_Face route proving FreeType stores the interface on the "
            "face and later uses it during glyph load across Rust FFI, C ABI, "
            "and WASM ABI"
        ),
        "ftincrem.FT_Incremental_InterfaceRec.object_round_trips_to_callbacks": (
            "FT_Incremental_InterfaceRec object parity needs callback event "
            "logs proving the client object pointer/value round-trips into "
            "glyph-data and metrics callbacks exactly like pinned C"
        ),
        "ftincrem.FT_Incremental_Metrics.null_not_passed_by_c": (
            "FT_Incremental_Metrics nullness parity needs a maintained metrics "
            "callback route proving pinned C never passes a null metrics "
            "pointer for glyphs where it requests overrides"
        ),
        "ftincrem.FT_Incremental_MetricsRec.input_metrics_seed_matches_c": (
            "FT_Incremental_MetricsRec seed parity needs exact callback-input "
            "metrics captured before mutation and compared with pinned C "
            "horizontal and vertical seed values"
        ),
        "ftincrem.FT_Incremental_MetricsRec.horizontal_override_applied": (
            "FT_Incremental_MetricsRec horizontal override parity needs a "
            "glyph-load route where callback-written metrics alter public "
            "advance/bearing output exactly like pinned C across all ABI lanes"
        ),
        "ftincrem.FT_Incremental_MetricsRec.vertical_override_applied_where_c_calls_it": (
            "FT_Incremental_MetricsRec vertical override parity needs a "
            "fixture where pinned C calls the metrics callback for vertical "
            "metrics and public vertical advances/bearings are compared "
            "exactly across Rust FFI, C ABI, and WASM ABI"
        ),
    }
    reason = ftincrem_rows_without_maintained_route.get(row.case_id)
    if reason is None:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    return reason


def done_glyph_lifecycle_pending_reason(row: ConcreteInput) -> str | None:
    """Case-specific non-null FT_Done_Glyph rows that need owned glyph routing."""
    if row.operation != "ftglyph.done_glyph":
        return None
    if row.case_id == "ftglyph.FT_OutlineGlyphRec.owns_outline_arrays":
        if {"glyph_index", "load_flags"} <= row.params.keys() and (
            "font" in row.assets or "outline_font" in row.assets
        ):
            return None
    if (
        row.case_id == "ftglyph.FT_Done_Glyph.success_null_is_noop"
        and row.params.get("glyph") is None
    ):
        return None
    if (
        row.case_id == "ftglyph.FT_Done_Glyph.outline_glyph_before_library_done"
        and row.params.get("destroy_order") == "glyph_before_face_and_library"
        and "outline_font" in row.assets
    ):
        return None
    if (
        row.case_id == "ftglyph.FT_Done_Glyph.success_releases_owned_outline_glyph"
        and row.params.get("creation_path") == "FT_Get_Glyph outline"
        and row.params.get("destroy_order") == "glyph_before_face_and_library"
        and {"glyph_index", "load_flags"} <= row.params.keys()
        and "outline_font" in row.assets
    ):
        return None
    if (
        row.case_id == "ftglyph.FT_Done_Glyph.success_releases_owned_bitmap_glyph"
        and row.params.get("creation_path") == "FT_Get_Glyph bitmap"
        and row.params.get("destroy_with") == "FT_Done_Glyph"
        and "bitmap_strike_font" in row.assets
    ):
        return None
    if row.case_id == "ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer":
        creation_paths = row.params.get("creation_paths")
        if (
            isinstance(creation_paths, list)
            and {"FT_Get_Glyph bitmap", "FT_Glyph_To_Bitmap outline"}
            <= set(creation_paths)
            and "outline_font" in row.assets
            and "bitmap_strike_font" in row.assets
        ):
            return None
    pending_cases = {
        "fterrdef.FT_Err_Invalid_Handle.generic_object_handle_validation": (
            "FT_Done_Glyph invalid-handle parity needs a maintained owned-glyph "
            "facade that distinguishes valid glyphs, null no-op, and foreign "
            "or stale handles without treating any non-null pointer as valid"
        ),
        "ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer": (
            "FT_BitmapGlyphRec ownership parity needs a maintained bitmap-glyph "
            "route proving the bitmap buffer is owned by the glyph and released "
            "by FT_Done_Glyph exactly like pinned C"
        ),
        "ftglyph.FT_Done_Glyph.success_releases_owned_glyph": (
            "FT_Done_Glyph success parity needs a maintained owned-glyph "
            "allocation/free route proving a real glyph is released once across "
            "pinned C, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftglyph.FT_Done_Glyph.lifetime_before_library_done": (
            "FT_Done_Glyph lifetime parity needs a maintained glyph/library "
            "route proving owned glyphs can be released before FT_Done_Library "
            "with the same allocator and handle invalidation behavior as pinned C"
        ),
        "ftglyph.FT_OutlineGlyphRec.owns_outline_arrays": (
            "FT_OutlineGlyphRec ownership parity needs a maintained outline-glyph "
            "route proving contour, point, and tag arrays are glyph-owned and "
            "released by FT_Done_Glyph exactly like pinned C"
        ),
    }
    return pending_cases.get(row.case_id)


def ftglyph_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for glyph object behavior that do not have a maintained route."""
    done_glyph_pending = done_glyph_lifecycle_pending_reason(row)
    if done_glyph_pending:
        return done_glyph_pending
    glyph_object_rows_without_exact_payload_route = {
        "ftglyph.FT_Get_Glyph.success_svg_slot_deep_copy": (
            "FT_Get_Glyph SVG success parity needs an SVG-enabled glyph route "
            "that creates FT_GLYPH_FORMAT_SVG and compares FT_SvgGlyphRec "
            "document/metrics/range/transform fields; slot format/advance "
            "alone is not the declared output"
        ),
        "ftglyph.FT_Glyph_Copy.success_svg_copy_is_independent": (
            "FT_Glyph_Copy SVG success parity needs an SVG-enabled glyph copy "
            "route proving document bytes, metrics, glyph range, transform, "
            "and delta are copied exactly or classified as unsupported like "
            "pinned C"
        ),
        "ftglyph.FT_SvgGlyphRec.fields_match_svg_get_copy_transform": (
            "FT_SvgGlyphRec field parity needs an SVG-enabled FT_Get_Glyph, "
            "FT_Glyph_Copy, and FT_Glyph_Transform route comparing the public "
            "SVG record payload; generic glyph root snapshots are not enough"
        ),
    }
    reason = glyph_object_rows_without_exact_payload_route.get(row.case_id)
    if reason is not None:
        return reason
    ftglyph_rows_without_maintained_route = {
        "ftglyph.FT_Glyph.caller_owned_lifetime": (
            "FT_Glyph caller-owned lifetime parity needs a maintained "
            "allocation/free event route for FT_New_Glyph, FT_Get_Glyph, "
            "FT_Glyph_Copy, FT_Glyph_To_Bitmap, and FT_Done_Glyph; treating "
            "the handle as an opaque non-null pointer would be a green placeholder"
        ),
        "ftglyph.FT_Glyph_Class.opaque_class_identity_only": (
            "FT_Glyph_Class identity parity must create outline, bitmap, and "
            "SVG glyphs through public operations and classify the private "
            "class pointer only by stable public behavior; raw private pointer "
            "or field comparison is not portable C/Rust/C-ABI/WASM parity"
        ),
        "ftglyph.FT_Glyph_Transform.success_svg_transform_accumulates": (
            "FT_Glyph_Transform SVG parity needs an SVG-enabled glyph route "
            "that accumulates transform and delta into FT_SvgGlyphRec exactly "
            "like pinned C; outline transform success does not prove SVG record "
            "mutation"
        ),
        "ftglyph.FT_New_Glyph.success_renderer_supported_custom_format": (
            "FT_New_Glyph custom-format success parity needs a maintained "
            "synthetic renderer registration route where pinned C accepts the "
            "format, initializes the glyph class payload, and exposes matching "
            "root fields and ownership across Rust FFI, C ABI, and WASM ABI"
        ),
        "ftglyph.FT_SvgGlyph.pointer_alias_matches_record_when_enabled": (
            "FT_SvgGlyph alias parity needs an SVG-enabled fixture where "
            "FT_Get_Glyph returns FT_GLYPH_FORMAT_SVG and the cast record "
            "matches pinned C FT_SvgGlyphRec fields through Rust FFI, C ABI, "
            "and WASM ABI"
        ),
        "ftglyph.FT_SvgGlyph.feature_availability_recorded": (
            "FT_SvgGlyph feature availability parity needs a maintained build "
            "feature probe that distinguishes enabled SVG glyph records from "
            "unsupported builds for the same public SVG glyph input across all "
            "ABI lanes"
        ),
        "ftglyph.FT_SvgGlyphRec.svg_feature_disabled_classification": (
            "FT_SvgGlyphRec disabled-feature parity needs an exact same-input "
            "unsupported-SVG route that compares pinned C, Rust FFI, C ABI, "
            "and WASM ABI error/record classification; generic unsupported "
            "format handling is not sufficient"
        ),
    }
    reason = ftglyph_rows_without_maintained_route.get(row.case_id)
    if reason is None:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    return reason


def ftparams_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for FT_Open_Args parameters that do not have a maintained route."""
    ftparams_rows_without_maintained_route = {
        "ftparams.FT_PARAM_TAG_IGNORE_SBIX.open_face_ignores_sbix": (
            "FT_PARAM_TAG_IGNORE_SBIX must be verified with a real C-openable "
            "SBIX font that has an outline/default-strike distinction through "
            "pinned C FreeType sfnt/sfobjs.c parameter dispatch, Rust FFI, "
            "thin C ABI FT_Open_Face params, and WASM ABI; the existing "
            "non-SBIX no-effect row does not prove SBIX outline/bitmap "
            "selection"
        ),
        "ftparams.FT_PARAM_TAG_IGNORE_SBIX.bitmap_only_requires_real_sbix_fixture": (
            "FT_PARAM_TAG_IGNORE_SBIX bitmap-only behavior needs a maintained "
            "real SBIX fixture and exact C/Rust/C-ABI/WASM comparison of the "
            "bitmap-only or missing-outline result; current sbix-named assets "
            "are not sufficient proof, so counting this would be a green "
            "placeholder"
        ),
        "ftparams.FT_PARAM_TAG_INCREMENTAL.incremental_interface_used_for_glyph_load": (
            "FT_PARAM_TAG_INCREMENTAL success parity needs a maintained "
            "incremental-font route that stores the callback interface during "
            "FT_Open_Face, invokes glyph-data callbacks, releases glyph data, "
            "applies metrics overrides, and compares callback event logs and "
            "public glyph output across pinned C, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftparams.FT_PARAM_TAG_RANDOM_SEED.valid_seed_sets_face_property": (
            "FT_PARAM_TAG_RANDOM_SEED valid-data parity needs a maintained "
            "route that proves the seeded face-internal value through a "
            "driver-visible public CFF/Type1/CID output or a pinned C oracle "
            "showing no observable output change across Rust FFI, C ABI, and "
            "WASM ABI; scalar error handling alone is not enough"
        ),
        "ftparams.FT_PARAM_TAG_STEM_DARKENING.cff_type1_toggle_changes_supported_output": (
            "FT_PARAM_TAG_STEM_DARKENING output parity needs a C-openable "
            "CFF/Type1/CID fixture where toggling the property changes or "
            "provably preserves a public metric, outline, or bitmap result "
            "across pinned C, Rust FFI, C ABI, and WASM ABI; the existing "
            "null-data/scalar property route does not prove output behavior"
        ),
    }
    reason = ftparams_rows_without_maintained_route.get(row.case_id)
    if reason is None:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    return reason


def ftparams_name_option_real_parity_reason(row: ConcreteInput) -> str | None:
    """Contained FT_Open_Args name-option rows with null data and maintained routes."""
    if row.case_id in {
        "ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY.open_face_uses_legacy_family_name",
        "ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY.null_data_accepted",
        "ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY.open_face_uses_legacy_subfamily_name",
        "ftparams.FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY.null_data_accepted",
    }:
        return (
            "FT_Open_Args typographic-name option with null data validates through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    return None


def ftparams_ignored_param_real_parity_reason(row: ConcreteInput) -> str | None:
    """Deprecated/no-effect FT_Open_Args parameter rows with maintained routes."""
    if row.case_id in {
        "ftparams.FT_PARAM_TAG_IGNORE_SBIX.unsupported_or_non_sbix_no_spurious_failure",
        "ftparams.FT_PARAM_TAG_UNPATENTED_HINTING.open_face_no_effect",
        "ftparams.FT_PARAM_TAG_UNPATENTED_HINTING.null_data_accepted_or_ignored",
    }:
        if row.case_id.startswith("ftparams.FT_PARAM_TAG_IGNORE_SBIX."):
            return (
                "FT_Open_Args SBIX-ignore parameter validates as no-effect on "
                "a non-SBIX SFNT through pinned C oracle, Rust FFI output, "
                "C ABI FT_Open_Face params, and WASM ABI output"
            )
        return (
            "FT_Open_Args deprecated unpatented-hinting parameter validates as "
            "no-effect through pinned C oracle, Rust FFI output, C ABI "
            "FT_Open_Face params, and WASM ABI output"
        )
    return None


def ftimage_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for image/raster public records that do not have a maintained route."""
    if row.case_id == "ftimage.FT_Pos.coordinate_outputs_use_ft_pos":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Pos coordinate endpoint parity declares synthetic outline "
            "outlines/synthetic/negative-and-large-coordinates.json, but that "
            "maintained outline asset is absent; exact same-input parity also "
            "requires a coordinate endpoint route for FT_Load_Glyph outline "
            "points, FT_Outline_Get_CBox, FT_Vector_Transform, and "
            "FT_Outline_Decompose across Rust FFI, C ABI, and WASM ABI; "
            "keeping this generic would be a green placeholder"
        )

    ftimage_rows_without_maintained_route = {
        "ftimage.FT_GLYPH_FORMAT_PLOTTER.source_emitter_inventory": (
            "FT_GLYPH_FORMAT_PLOTTER success parity needs a maintained glyph "
            "format emitter route or pinned C evidence that no shipped module "
            "can produce plotter glyphs; scalar tag equality alone does not "
            "prove runtime source inventory across Rust FFI, C ABI, and WASM ABI"
        ),
        "ftimage.FT_GLYPH_FORMAT_SVG.produced_by_svg_glyph_load_when_enabled": (
            "FT_GLYPH_FORMAT_SVG runtime parity needs an SVG-enabled pinned C "
            "fixture and pure-Rust SVG glyph-slot route that compares load "
            "error, slot format, SVG document fields, and C/WASM ABI output; "
            "unsupported or scalar-format checks do not prove produced glyphs"
        ),
        "ftimage.FT_GLYPH_FORMAT_SVG.unsupported_svg_build_classification": (
            "FT_GLYPH_FORMAT_SVG unsupported-build parity needs a maintained "
            "build-feature classification route that compares pinned C, Rust "
            "FFI, C ABI, and WASM ABI behavior for the same SVG glyph input; "
            "treating any load error as equivalent would be a green placeholder"
        ),
        "ftimage.FT_OUTLINE_OWNER.destruction_ownership_behavior": (
            "FT_OUTLINE_OWNER lifecycle parity needs a maintained outline "
            "allocation/free route that proves owner-bit destruction semantics "
            "and allocator ownership across pinned C, Rust FFI, C ABI, and "
            "WASM ABI; generic drop behavior would not prove the public flag"
        ),
        "ftimage.FT_Raster.lifecycle_callback_contract": (
            "FT_Raster lifecycle parity needs a maintained custom renderer "
            "facade that records raster object allocation, reset, render, "
            "set-mode, and done callback ordering across pinned C, Rust FFI, "
            "C ABI, and WASM ABI"
        ),
        "ftimage.FT_Raster_Done_Func.renderer_lifecycle_calls_done": (
            "FT_Raster_Done_Func parity needs a maintained custom renderer "
            "lifecycle route proving FreeType calls the done callback with the "
            "expected raster handle and teardown ordering across all ABI lanes"
        ),
        "ftimage.FT_Raster_Funcs.callback_slots_match_registered_renderers": (
            "FT_Raster_Funcs callback-slot parity needs a maintained renderer "
            "class probe that registers synthetic renderers and compares the "
            "public callback table identity/availability across C, Rust FFI, "
            "C ABI, and WASM ABI"
        ),
        "ftimage.FT_Raster_New_Func.renderer_lifecycle_calls_new": (
            "FT_Raster_New_Func parity needs a maintained custom renderer "
            "route proving the new callback, raster handle creation, and "
            "failure cleanup behavior against pinned C"
        ),
        "ftimage.FT_Raster_Reset_Func.renderer_lifecycle_calls_reset": (
            "FT_Raster_Reset_Func parity needs a maintained renderer route "
            "that records reset callback invocation and memory-pool argument "
            "semantics across pinned C, Rust FFI, C ABI, and WASM ABI"
        ),
        "ftimage.FT_Raster_Set_Mode_Func.set_mode_result_is_observable": (
            "FT_Raster_Set_Mode_Func parity needs a maintained custom renderer "
            "route where set-mode return codes and side effects are observable "
            "through subsequent public render behavior across all ABI lanes"
        ),
        "ftimage.FT_Raster_Span_Func.direct_render_emits_spans": (
            "FT_Raster_Span_Func direct-render parity needs a maintained "
            "outline render route that compares emitted span count, y/x/len/"
            "coverage tuples, clipping, and callback ordering against pinned C "
            "through Rust FFI, C ABI, and WASM ABI"
        ),
    }
    reason = ftimage_rows_without_maintained_route.get(row.case_id)
    if reason is None:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    return reason


def freetype_core_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for core FreeType face/size/slot behavior without a maintained route."""
    if row.case_id == "freetype.FT_LOAD_SVG_ONLY.svg_only_behavior":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_LOAD_SVG_ONLY runtime parity declares fonts/svg/color-svg-glyph.ttf, "
            "but that maintained OT-SVG font fixture is absent; exact same-input "
            "C/Rust/C-ABI/WASM parity also requires a load-glyph route that "
            "compares SVG-glyph success and non-SVG-glyph public error behavior "
            "instead of reusing the constant-value flag route"
        )
    if row.case_id == "freetype.FT_Parameter.tag_data_parameters_match_c_behavior":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Parameter runtime parity declares fonts/color/sbix-outline.ttf "
            "for FT_PARAM_TAG_IGNORE_SBIX, but that maintained sbix fixture is "
            "absent; exact same-input C/Rust/C-ABI/WASM parity also requires "
            "FT_Open_Face with FT_OPEN_PARAMS/FT_Parameter dispatch to compare "
            "known-tag, unknown-tag, null-data, and null-params behavior"
        )
    if row.case_id == "freetype.FT_Attach_File.success_attach_auxiliary_file":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        if row.operation == "freetype.attach_file" and not row.expect_error:
            return None
        return (
            "FT_Attach_File success requires the declared C-openable Type1 "
            "PFA/PFB face plus matching AFM/PFM pathname asset and a maintained "
            "attach route that compares the FT_Attach_File return code and "
            "post-attach kerning/track-kerning mutations across pinned C, Rust "
            "FFI, C ABI, and WASM; unsupported-file and null-path errors do not "
            "prove auxiliary attachment success"
        )
    if row.case_id == "freetype.FT_Attach_Stream.success_attach_auxiliary_stream":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        if row.operation == "freetype.attach_stream" and not row.expect_error:
            return None
        return (
            "FT_Attach_Stream success requires the declared C-openable Type1 "
            "PFA/PFB face plus matching AFM/PFM bytes and a maintained "
            "FT_Open_Args/FT_OPEN_MEMORY stream route that compares return code, "
            "stream ownership, and post-attach kerning/track-kerning mutations "
            "across pinned C, Rust FFI, C ABI, and WASM; null/invalid args "
            "coverage does not exercise driver attach behavior"
        )
    if row.case_id == "freetype.FT_FaceRec.populated_public_fields_match_c":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_FaceRec populated-field parity requires splitting the current "
            "broad snapshot into concrete C-openable operation stages: initial "
            "face fields, size mutation, glyph load, charmap selection, "
            "auxiliary attachment, and variation mutation. The row still names "
            "missing bitmap and Type1 auxiliary assets, so treating a partial "
            "inspect_face_rec route as full public-record parity would be a "
            "green placeholder"
        )
    if row.case_id == "freetype.FT_Get_Track_Kerning.type1_afm_track_kerning_success":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        if row.operation == "freetype.get_track_kerning" and not row.expect_error:
            return None
        return (
            "FT_Get_Track_Kerning Type1/AFM success requires maintained "
            "attach-first routing, pure-Rust AFM track-kerning state, and exact "
            "akerning comparisons for negative, zero, and positive track "
            "degrees over declared 16.16 point sizes across pinned C, Rust "
            "FFI, C ABI, and WASM; the generated Type1/AFM assets alone are "
            "not success parity"
        )
    if row.case_id == "freetype.FT_Open_Args.open_face_consumes_args_like_c":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Open_Args open-face behavior requires converting abstract "
            "arg_variants into explicit maintained variants[] rows consumed by "
            "the runner, then comparing FT_OPEN_MEMORY, driver, params, "
            "negative face-index probe, stream, and pathname behavior across "
            "pinned C, Rust FFI, C ABI, and WASM; current memory helpers alone "
            "do not prove the full argument-dispatch contract"
        )
    return None


def specialized_record_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for specialized public records without a maintained route."""
    specialized_rows_without_maintained_route = {
        "otsvg.FT_SVG_Document.renderer_callback_observes_document": (
            "FT_SVG_Document renderer-callback parity needs a maintained "
            "OT-SVG glyph route proving the renderer callback receives the "
            "same document pointer class, glyph ID, and lifetime as pinned C"
        ),
        "otsvg.FT_SVG_DocumentRec.document_range_and_payload_fields": (
            "FT_SVG_DocumentRec range parity needs a maintained OT-SVG route "
            "proving document start/end offsets, payload byte range, and "
            "document length fields match pinned C"
        ),
        "otsvg.FT_SVG_DocumentRec.transform_and_metrics_fields": (
            "FT_SVG_DocumentRec transform/metrics parity needs a maintained "
            "OT-SVG route proving transform matrix, delta, metrics, units, and "
            "glyph size fields are populated exactly like pinned C"
        ),
    }
    if row.case_id not in specialized_rows_without_maintained_route:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    return specialized_rows_without_maintained_route[row.case_id]


def stream_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for compressed and external stream behavior without a maintained route."""
    bzip2_stream_rows_without_maintained_route = {
        "ftbzip2.FT_Stream_OpenBzip2.error_null_stream_or_source": (
            "FT_Stream_OpenBzip2 null stream/source validation is enabled-build "
            "behavior. The active pinned oracle build disables bzip2 and returns "
            "Unimplemented_Feature before null validation, so this row must stay "
            "pending until split by build configuration or a bzip2-enabled oracle "
            "variant is maintained"
        ),
        "ftbzip2.FT_Stream_OpenBzip2.error_invalid_or_truncated_bzip2_header": (
            "FT_Stream_OpenBzip2 invalid/truncated header validation is enabled-build "
            "behavior. The active pinned oracle build disables bzip2 and returns "
            "Unimplemented_Feature before reading source bytes, so this row must stay "
            "pending until split by build configuration or a bzip2-enabled oracle "
            "variant is maintained"
        ),
        "ftbzip2.FT_Stream_OpenBzip2.success_open_valid_bzip2_stream": (
            "FT_Stream_OpenBzip2 open parity needs maintained compressed/raw "
            "fixtures plus a pure-Rust bzip2 stream route proving target stream "
            "fields, source position, and initial open status match "
            "freetype/src/bzip2/ftbzip2.c"
        ),
        "ftbzip2.FT_Stream_OpenBzip2.success_read_decompressed_bytes": (
            "FT_Stream_OpenBzip2 read parity needs maintained compressed/raw "
            "fixtures plus a pure-Rust bzip2 stream route proving decompressed "
            "byte ranges, stream positions, and read status match pinned C"
        ),
        "ftbzip2.FT_Stream_OpenBzip2.lifecycle_close_does_not_close_source": (
            "FT_Stream_OpenBzip2 close-lifecycle parity needs a maintained "
            "route proving closing the bzip2 wrapper releases wrapper state "
            "without closing or corrupting the caller-owned source stream"
        ),
        "ftbzip2.FT_Stream_OpenBzip2.out_of_scope_uncompiled_bzip2_policy": (
            "FT_Stream_OpenBzip2 build-policy parity needs a maintained route "
            "proving the active pinned build's compiled-bzip2 behavior instead "
            "of treating optional bzip2 support as out of scope"
        ),
    }
    if row.case_id in bzip2_stream_rows_without_maintained_route:
        if (
            row.case_id
            == "ftbzip2.FT_Stream_OpenBzip2.out_of_scope_uncompiled_bzip2_policy"
        ):
            return None
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return bzip2_stream_rows_without_maintained_route[row.case_id]
    if row.case_id == "ftgzip.FT_Gzip_Uncompress.uncompresses_valid_gzip_buffer":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Gzip_Uncompress success declares "
            "compressed/gzip/small-text-and-empty-payloads.json, but that "
            "maintained gzip/zlib payload manifest is absent; exact same-input "
            "C/Rust/C-ABI/WASM byte-output parity also requires a pure-Rust "
            "gzip/zlib success route instead of the current exact-error-only "
            "gzip routing"
        )
    if row.case_id == "ftgzip.FT_Stream_OpenGzip.opens_valid_gzip_stream":
        if row.operation == "ftgzip.stream_open_gzip":
            return None
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Stream_OpenGzip success declares "
            "compressed/gzip/small-and-large-streams.json, but that "
            "maintained gzip stream manifest is absent; exact same-input "
            "C/Rust/C-ABI/WASM stream parity also requires a pure-Rust gzip "
            "route that matches FreeType's below/above-40KiB in-memory versus "
            "callback stream behavior from freetype/src/gzip/ftgzip.c:608-708"
        )
    if row.case_id == "ftlzw.FT_Stream_OpenLZW.opens_valid_lzw_stream":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Stream_OpenLZW success declares streams/lzw/small-valid-pcf.Z "
            "and facades/stream/memory-ft-stream.json, but those maintained "
            "LZW byte/facade fixtures are absent; exact same-input "
            "C/Rust/C-ABI/WASM stream parity also requires a pure-Rust LZW "
            "route that matches open/read/backward-seek/close behavior from "
            "freetype/src/lzw/ftlzw.c:221-308 and 337-383"
        )
    if row.case_id == "ftsystem.FT_Memory.custom_allocator_runtime_events":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Memory custom allocator runtime parity declares "
            "memory/harnesses/custom-allocator-events.json, but that "
            "maintained allocator-event harness is absent; exact same-input "
            "C/Rust/C-ABI/WASM parity also requires FT_New_Library, "
            "FT_Add_Default_Modules, FT_New_Memory_Face, FT_Done_Face, and "
            "FT_Done_Library to preserve callback order and FT_Memory pointer "
            "identity from freetype/src/base/ftobjs.c:5472 and ftutil.c"
        )
    if row.case_id == "ftsystem.FT_Stream.external_stream_runtime_contract":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Stream external-stream runtime parity declares "
            "streams/harnesses/external-stream-errors.json, but that "
            "maintained read/seek-failure harness is absent; exact same-input "
            "C/Rust/C-ABI/WASM parity also requires FT_Open_Face with "
            "FT_OPEN_STREAM to preserve caller-owned stream identity and "
            "read/close callback events from freetype/src/base/ftobjs.c:2514"
        )
    if row.case_id == "ftsystem.FT_StreamRec.callback_stream_field_contract":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_StreamRec callback-stream parity declares "
            "streams/harnesses/external-stream-callbacks.json, but that "
            "maintained callback-event harness is absent; exact same-input "
            "C/Rust/C-ABI/WASM parity also requires public FT_StreamRec field "
            "copying and read(count==0), read(count>0), and close-event "
            "observation from freetype/include/freetype/ftsystem.h:325-340"
        )
    if row.case_id == "ftsystem.FT_StreamRec.memory_stream_field_contract":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_StreamRec memory-stream field parity uses the maintained "
            "input/fonts/DejaVuSans.ttf asset, but still needs a maintained "
            "memory-stream probe route that opens the same bytes with "
            "FT_New_Memory_Face and compares base/size/pos/cursor/limit plus "
            "frame-read events across Rust FFI, C ABI, and WASM; treating the "
            "layout-only FT_StreamRec route as runtime parity would be a green "
            "placeholder"
        )
    return None


def callback_provider_subsystem_pending_reason(row: ConcreteInput) -> str | None:
    """Rows for callback/provider ABI behavior without a maintained route."""
    if row.case_id == "ftrender.FT_Renderer_Class.render_mode_acceptance_matches_callbacks":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Renderer_Class render-mode acceptance requires a maintained "
            "renderer-behavior route that selects each public renderer class "
            "and compares FT_Render_Glyph errors plus bitmap descriptors/bytes "
            "for NORMAL, LIGHT, MONO, LCD, LCD_V, SDF, and SVG modes across "
            "pinned C, Rust FFI, C ABI, and WASM; class layout and "
            "FT_Get_Renderer metadata alone would be a green placeholder"
        )
    if row.case_id == "ftrender.FT_Set_Renderer.set_outline_renderer_success":
        if exact_error_public_route(row.operation, row.case_id, row.expect_error):
            return None
        return (
            "FT_Set_Renderer success requires a maintained renderer-list route "
            "matching freetype/src/base/ftobjs.c:4676-4732: validate the "
            "renderer belongs to library->renderers, move its node with "
            "FT_List_Up, update cur_renderer only for outline format, invoke "
            "clazz->set_mode for each parameter until first error, and compare "
            "renderer identity plus rendered output across pinned C, Rust FFI, "
            "C ABI, and WASM; treating existing invalid-argument/set-mode-error "
            "rows as success parity would be a green placeholder"
        )
    return None


def residual_public_surface_pending_reason(row: ConcreteInput) -> str | None:
    """Residual public data, error, outline, and table rows without a route."""
    if row.case_id == "ftotval.FT_OpenType_Free.frees_validated_table_with_face_memory":
        return (
            "FT_OpenType_Free after validation requires a maintained non-null "
            "FT_OpenType_Validate table buffer route and face-memory ownership "
            "tracking; current Rust FFI/C ABI/WASM coverage only proves the "
            "null face and null table no-op cases"
        )
    if row.case_id == "ftpfr.FT_Get_PFR_Metrics.pfr_metrics_success":
        return (
            "FT_Get_PFR_Metrics success requires a maintained C-openable PFR "
            "font with the PFR_METRICS service and a pure-Rust PFR metrics route; "
            "the existing non-PFR metrics/error routes do not prove outline "
            "resolution, metrics resolution, or before/after-size scale parity "
            "through pinned C, Rust FFI, C ABI, and WASM ABI"
        )

    residual_rows_without_maintained_route = {
        "fterrdef.FT_Err_Missing_Property.known_property_success",
        "ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output",
    }
    if row.case_id not in residual_rows_without_maintained_route:
        return None
    if exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return None
    return (
        "Residual CID, error/status, OpenType validation, outline orientation, "
        "PFR metric, and TrueType table behavior requires maintained public "
        "surface routes; keeping it generic would be a green placeholder"
    )


def malformed_maxp_fixture_pending_reason(row: ConcreteInput) -> str | None:
    """Malformed maxp rows that still need an executable same-input route."""
    if row.case_id != "tttables.TT_MaxProfile.malformed_table_error_source":
        return None
    return None


def otvalid_expectation_mismatch_pending_reason(row: ConcreteInput) -> str | None:
    """OpenType validation rows whose fixture expectation disagrees with pinned C."""
    if row.case_id != "ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output":
        return None
    return (
        "Fixture expects FT_VALIDATE_BASE absent-table success/null output, but "
        "the pinned FreeType oracle used by this harness returns "
        "FT_Err_Unimplemented_Feature (7) before table absence can be observed; "
        "do not promote without updating the fixture expectation or oracle build "
        "contract"
    )


def otvalid_unresolved_asset_pending_reason(row: ConcreteInput) -> str | None:
    """OpenType validation rows cannot be real parity without their declared fixture."""
    if row.operation != "ftotval.open_type_validate":
        return None
    if (
        row.case_id == "ftotval.FT_OpenType_Validate.selected_tables_success"
        and not (FIXTURE_DIR / "fonts/opentype/valid-all-layout.otf").is_file()
    ):
        return (
            "font references missing fixture fonts/opentype/valid-all-layout.otf; "
            "FT_OpenType_Validate cannot be counted as real same-input "
            "C/Rust/C-ABI/WASM parity until the declared OpenType validation "
            "fixture exists"
        )
    unresolved = unresolved_assets_reason(row)
    if not unresolved:
        return None
    return (
        f"{unresolved}; FT_OpenType_Validate cannot be counted as real "
        "same-input C/Rust/C-ABI/WASM parity until the declared OpenType "
        "validation fixture exists"
    )


def operation_is_compile_contract(operation: str) -> bool:
    return operation in COMPILE_CONTRACT_OPERATIONS or operation.startswith(
        COMPILE_CONTRACT_PREFIXES
    )


def operation_is_real_parity(operation: str) -> bool:
    return operation in REAL_PARITY_OPERATIONS or operation.startswith(REAL_PARITY_PREFIXES)


def has_runtime_asset(row: ConcreteInput) -> bool:
    return any(key in row.assets for key in ("font", "fixture", "foreign_font"))


def unresolved_assets_reason(row: ConcreteInput) -> str | None:
    reasons = [
        unresolved_asset_reason(asset, name)
        for name, asset in sorted(row.assets.items())
    ]
    return next((reason for reason in reasons if reason), None)


def unresolved_asset_reason(value: object, label: str) -> str | None:
    if isinstance(value, list):
        for index, item in enumerate(value):
            reason = unresolved_asset_reason(item, f"{label}[{index}]")
            if reason:
                return reason
        return None
    if not isinstance(value, dict):
        return None
    for key, item in value.items():
        if key not in {"id", "path"}:
            reason = unresolved_asset_reason(item, f"{label}.{key}")
            if reason:
                return reason
    if value.get("kind") not in {"ref", "file"}:
        return None
    if value.get("optional") is True:
        return None
    if value.get("role") == "missing_path":
        return None
    reference = value.get("id") or value.get("path")
    if not isinstance(reference, str) or "/" not in reference:
        if value.get("status") == "required_future_asset":
            return f"{label} is marked required_future_asset"
        return None
    if not fixture_reference_exists(reference):
        if value.get("status") == "required_future_asset":
            return f"{label} is marked required_future_asset"
        return f"{label} references missing fixture {reference}"
    return None


def fixture_reference_exists(reference: str) -> bool:
    if (FIXTURE_DIR / reference).is_file():
        return True
    aliases = {
        # Public input manifests preserve the historical logical fixture id.
        # The maintained BDF runtime asset lives under tests/fixtures/input.
        "fonts/bdf/properties-atoms-integers-cardinals.bdf": (
            "input/fonts/bdf/properties-atoms-integers-cardinals.bdf"
        ),
        # Historical logical id for the maintained COLRv1 all-paints fixture.
        "fonts/color/colr_v1_all_paint_formats.ttf": (
            "fonts/color/colr-v1-all-paints.ttf"
        ),
        # Historical logical id for the maintained COLRv1 root-transform fixture.
        "fonts/color/colr-v1-root-paint-cpal.ttf": (
            "fonts/color/colr-v1-root-transform.ttf"
        ),
    }
    alias = aliases.get(reference)
    return alias is not None and (FIXTURE_DIR / alias).is_file()


def pending_route_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftbdf.get_bdf_charset_id":
        if row.case_id in {
            "ftbdf.FT_Get_BDF_Charset_ID.error_non_bdf_face",
            "ftbdf.FT_Get_BDF_Charset_ID.success_bdf_face_charset",
            "ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs",
        }:
            return None
        unresolved = unresolved_assets_reason(row)
        if unresolved:
            return (
                f"{unresolved}; FT_Get_BDF_Charset_ID cannot count as real "
                "parity until the BDF/SFNT-BDF fixture is present and the "
                "same-input Rust FFI, C ABI, and WASM ABI routes compare exact "
                "pinned FreeType output"
            )
        return (
            "FT_Get_BDF_Charset_ID has fixture rows but no maintained Rust FFI, "
            "C ABI, or WASM runtime route; classifying charset output as real "
            "parity would be a green placeholder"
        )
    if row.operation == "ftbdf.get_bdf_property":
        if row.case_id in {
            "ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties",
            "ftbdf.FT_Get_BDF_Property.error_missing_property_sets_none",
            "ftbdf.FT_Get_BDF_Property.error_null_face_or_output",
            "ftbdf.FT_Get_BDF_Property.error_unsupported_face_or_unselected_strike",
        }:
            return None
        unresolved = unresolved_assets_reason(row)
        if unresolved:
            return (
                f"{unresolved}; FT_Get_BDF_Property cannot count as real "
                "parity until the same input is available to pinned C, Rust "
                "FFI, C ABI, and WASM ABI"
            )
        return (
            "FT_Get_BDF_Property exact-error rows have a maintained native "
            "oracle and Rust FFI/C ABI/WASM ABI route, but this success row is "
            "still unresolved; PCF/SFNT-BDF success assets remain unavailable. "
            "Classifying this BDF property success output as real parity would "
            "be a green placeholder"
        )
    if not operation_is_real_parity(row.operation):
        return None
    if row.case_id == "ftglyph.FT_Glyph_Copy.error_copy_hook_failure_cleans_target":
        return (
            "FT_Glyph_Copy copy-hook failure cleanup requires a maintained "
            "allocation-failure facade and malformed glyph-class hook route; "
            "the existing null source/target/class route does not exercise "
            "partial target allocation, copy-hook failure, target destruction, "
            "or cleanup-event parity across pinned C, Rust FFI, C ABI, and WASM ABI"
        )
    unresolved = unresolved_assets_reason(row)
    if unresolved and not exact_error_public_route(row.operation, row.case_id, row.expect_error):
        return f"{unresolved}; exact runtime parity would be a green placeholder"
    unresolved_future_asset_cases = {
        "ftcache.FTC_SBitCache_Lookup.missing_bitmap_has_null_buffer": (
            "tracked cache bitmap strike asset is not a C-openable success fixture; "
            "pinned C returns error 6, so exact success would be a green placeholder"
        ),
        "fterrdef.FT_Err_Name_Table_Missing.sfnt_name_storage_out_of_bounds": (
            "generated bad-storage name table returns pinned-C public error 3, not "
            "FT_Err_Name_Table_Missing; exact Name_Table_Missing parity needs a "
            "different C-observable fixture"
        ),
        "fterrdef.FT_Err_Name_Table_Missing.sfnt_without_name_table": (
            "generated no-name-table SFNT opens successfully in pinned C; exact "
            "Name_Table_Missing parity needs a fixture that reaches ttload.c's "
            "public error path"
        ),
        "freetype.FT_ENCODING_NONE.representative_runtime_observation": (
            "tracked FT_ENCODING_NONE font is not a C-openable encoding-none fixture; "
            "pinned C returns error 23, so exact runtime observation would be a green placeholder"
        ),
        "freetype.FT_HAS_HORIZONTAL.no_horizontal_metrics_control": (
            "tracked no-horizontal-metrics control font is not C-openable for this macro; "
            "pinned C returns error 85, so exact macro success would be a green placeholder"
        ),
        "freetype.FT_Face_Properties.error_null_face": (
            "pinned FreeType 2.14.3 FT_Face_Properties dereferences face for "
            "num_properties > 0 and segfaults on a null face; counting a Rust "
            "Invalid_Face_Handle as C parity would be a green placeholder"
        ),
        "ftotval.FT_VALIDATE_BASE.absent_table_returns_null_output": (
            "pinned FreeType 2.14.3 FT_OpenType_Validate returns "
            "Unimplemented_Feature (7) for FT_VALIDATE_BASE in this build and "
            "leaves non-null output sentinels untouched; counting the declared "
            "OK/null-output expectation as parity would be a green placeholder"
        ),
    }
    if row.case_id in unresolved_future_asset_cases:
        return unresolved_future_asset_cases[row.case_id]
    exact_error_route_gaps = {
        (
            "ftoutln.outline_decompose",
            "ftimage.FT_Outline.invalid_outline_errors",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Bad_Argument.bytecode_invalid_jump_returns_error",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Code_Overflow.bytecode_jump_past_range_returns_error",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Code_Overflow.push_instruction_truncation_returns_error",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Corrupted_Font_Header.autohint_zero_units_per_em_returns_error",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Could_Not_Find_Context.truetype_context_allocation_failure_returns_error",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_DEF_In_Glyf_Bytecode.glyph_program_fdef_returns_error",
        ),
        ("load_glyph", "fterrdef.FT_Err_Debug_OpCode.debug_opcode_returns_error"),
        ("load_glyph", "fterrdef.FT_Err_Divide_By_Zero.bytecode_div_zero_returns_error"),
        ("load_glyph", "fterrdef.FT_Err_ENDF_In_Exec_Stream.stray_endf_returns_error"),
        (
            "load_glyph",
            "fterrdef.FT_Err_Execution_Too_Long.opcode_counter_limit_returns_error",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Execution_Too_Long.negative_jump_limit_returns_error",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Glyph_Too_Big.ps_builder_large_outline_returns_error",
        ),
        ("load_glyph", "fterrdef.FT_Err_Invalid_Opcode.tt_bytecode_invalid_opcode"),
        (
            "load_glyph",
            "fterrdef.FT_Err_Invalid_Reference.tt_bytecode_invalid_point_reference",
        ),
        ("load_glyph", "fterrdef.FT_Err_Nested_DEFS.truetype_nested_fdef"),
        ("load_glyph", "fterrdef.FT_Err_Nested_DEFS.truetype_nested_idef"),
        ("load_glyph", "fterrdef.FT_Err_Stack_Overflow.tt_interpreter_stack_overflow"),
        ("load_glyph", "fterrdef.FT_Err_Stack_Overflow.cff_charstring_stack_overflow"),
        (
            "load_glyph",
            "fterrdef.FT_Err_Stack_Underflow.cff_charstring_missing_operands",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Syntax_Error.charstring_or_afm_syntax_error",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Too_Few_Arguments.tt_interpreter_argument_underflow",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Too_Few_Arguments.cff_decoder_underflow_translation",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Too_Many_Function_Defs.tt_fdef_limit_exceeded",
        ),
        ("load_glyph", "fterrdef.FT_Err_Too_Many_Hints.tt_glyph_hint_limit"),
        (
            "load_glyph",
            "fterrdef.FT_Err_Too_Many_Instruction_Defs.tt_idef_limit_exceeded",
        ),
        (
            "load_glyph",
            "fterrdef.FT_Err_Unimplemented_Feature.unsupported_font_feature",
        ),
    }
    if (
        row.operation == "ftoutln.outline_decompose"
        and row.case_id == "ftimage.FT_Outline.invalid_outline_errors"
    ):
        return (
            "raw FT_Outline null-internal-pointer scenarios are not safely routed; "
            "the attempted pinned-C FT_Outline_Decompose probe segfaulted, so "
            "accepting a generic Invalid_Outline would be a green placeholder"
        )
    done_glyph_pending = done_glyph_lifecycle_pending_reason(row)
    if done_glyph_pending:
        return done_glyph_pending
    if row.case_id == "ftglyph.FT_Get_Glyph.error_unsupported_format_or_bad_slot_payload":
        return (
            "FT_Get_Glyph unsupported-format/bad-slot payload requires a "
            "maintained synthetic FT_GlyphSlot route matching "
            "freetype/src/base/ftglyph.c:633-682: construct slot->library, "
            "slot->format, and payload fields, call FT_New_Glyph plus the "
            "selected class glyph_init hook, then compare FT_Error and "
            "*aglyph null/preservation across pinned C, Rust FFI, C ABI, and "
            "WASM; accepting a generic slot error would be a green placeholder"
        )
    if row.case_id == "ftglyph.FT_Get_Glyph.error_advance_out_of_16_16_range":
        return None
    if row.case_id == "ftglyph.FT_GlyphRec.clazz_is_private_identity_only":
        return (
            "FT_GlyphRec clazz identity requires a maintained record-inspection "
            "route that creates outline, bitmap, and SVG glyphs through public "
            "C-observable operations and classifies the private clazz pointer "
            "only by public behavior and glyph format, matching "
            "freetype/include/freetype/ftglyph.h:93-120 across Rust FFI, C ABI, "
            "and WASM; raw pointer identity or layout-only checks would be a "
            "green placeholder"
        )
    if (
        (row.operation, row.case_id) in exact_error_route_gaps
        and (row.operation, row.case_id) not in FTERRDEF_EXACT_ERROR_BATCH
    ):
        return (
            "exact public error comparison is not routed; accepting any error "
            "would be a green placeholder"
        )
    existing_primary_font_cases = {
        "freetype.FT_Face_CheckTrueTypePatents.non_truetype_face_result",
        "freetype.FT_Get_FSType_Flags.sfnt_installable_embedding",
        "freetype.FT_Get_FSType_Flags.sfnt_restricted_embedding_bits",
        "freetype.FT_Get_First_Char.charcode_zero_disambiguated_by_glyph_index",
        "freetype.FT_HAS_FIXED_SIZES.bitmap_strike_font_true",
        "freetype.FT_HAS_GLYPH_NAMES.cid_keyed_cff_false",
        "freetype.FT_HAS_GLYPH_NAMES.glyph_names_font_true",
        "freetype.FT_HAS_GLYPH_NAMES.no_glyph_names_control_false",
        "freetype.FT_Get_Char_Index.active_charmap_present_and_missing_codes",
        "freetype.FT_New_Face.success_negative_face_index_probe",
        "freetype.FT_New_Memory_Face.success_named_instance_index",
        "freetype.FT_Open_Face.error_unknown_format_or_out_of_range_face",
        "freetype.FT_Open_Face.success_open_variation_named_instance",
        "freetype.FT_Request_Size.success_bitmap_request_match",
        "freetype.FT_Select_Charmap.success_select_present_encoding",
    }
    ft_list_topology_runner_cases = {
        "ftlist.FT_List_Insert.insert_empty_list",
        "ftlist.FT_List_Insert.insert_non_empty_list",
        "ftlist.FT_List_Insert.null_list_or_node_noop",
        "ftlist.FT_List_Remove.remove_head_middle_tail",
        "ftlist.FT_List_Remove.remove_only_node",
        "ftlist.FT_List_Remove.null_list_or_node_noop",
        "ftlist.FT_List_Remove.membership_not_checked",
        "ftlist.FT_List_Up.move_tail_or_middle_to_head",
        "ftlist.FT_List_Up.already_head_noop",
        "ftlist.FT_List_Up.null_list_or_node_noop",
    }
    for name, asset in sorted(row.assets.items()):
        reason = unresolved_asset_reason(asset, name)
        if (
            reason == "list_facade references missing fixture facades/list/ft-list-topologies.json"
            and row.case_id in ft_list_topology_runner_cases
        ):
            continue
        if (
            reason
            == "malformed_facade references missing fixture facades/glyph/malformed-slot-and-class-cases.json"
            and row.operation == "ftglyph.glyph_to_bitmap"
            and row.case_id
            == "ftglyph.FT_Glyph_To_Bitmap.error_invalid_arguments_or_unrenderable_format"
            and row.params.get("null_the_glyph") is True
            and row.params.get("null_deref_glyph") is True
            and row.params.get("null_library") is True
            and row.params.get("null_clazz") is True
            and row.params.get("no_prepare_hook") is True
        ):
            continue
        if (
            reason in {
                "font is marked required_future_asset",
                "font_bytes is marked required_future_asset",
                "malformed is marked required_future_asset",
                "non_unicode_charmap_font is marked required_future_asset",
                "variable_font is marked required_future_asset",
            }
            and row.case_id in existing_primary_font_cases
        ):
            continue
        if reason:
            return reason
    return None


def lifecycle_handle(row: ConcreteInput, name: str) -> str | None:
    value = row.params.get(name)
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        raw = value.get("handle") or value.get("value")
        return raw if isinstance(raw, str) else None
    return None


def param_is_null(row: ConcreteInput, name: str) -> bool:
    return name in row.params and row.params[name] is None or lifecycle_handle(row, name) == "null"


def lifecycle_handle_is_null(row: ConcreteInput, name: str) -> bool:
    handle = lifecycle_handle(row, name)
    return isinstance(handle, str) and handle.lower() == "null"


def has_null_lifecycle_handle(row: ConcreteInput) -> bool:
    return any(
        lifecycle_handle(row, name) == "null"
        for name in ("face", "library", "slot", "glyph_slot", "pathname", "aface")
    )


def pending_core_reason(row: ConcreteInput) -> str | None:
    if (
        row.operation
        in {
            "ftsynth.glyphslot_adjust_weight_after_load",
            "ftsynth.glyphslot_embolden_after_load",
        }
        and "bitmap_strike_font" in row.assets
    ):
        return (
            "bitmap glyph-slot synthesis requires FT_GlyphSlot_AdjustWeight "
            "bitmap-slot behavior plus an executable embedded-bitmap strike route"
        )
    if (
        row.operation == "ftsnames.get_sfnt_name"
        and row.expectation_status == "build_dependent"
        and lifecycle_handle(row, "face") == "non_sfnt"
    ):
        return "non-SFNT face fixture must open before FT_Get_Sfnt_Name"
    if row.operation != "ftmm.set_named_instance":
        return None
    if row.case_id == "ftmm.FT_Set_Named_Instance.success_adobe_mm_resets_default":
        return None
    if any(
        isinstance(item, dict) and item.get("operation") == "FT_Set_MM_Design_Coordinates"
        for item in list_value(row.params.get("prior_calls"))
    ):
        return "Adobe MM named-instance reset requires real Adobe MM support"
    if (
        row.case_id != "ftmm.FT_Set_Named_Instance.output_changes_to_named_instance"
        and "glyph_index" in row.params
    ):
        return "named-instance glyph-output parity requires fractional gvar/autohint bitmap support"
    return None


def size_null_validation_reason(row: ConcreteInput) -> str | None:
    if row.operation == "new_memory_face" and lifecycle_handle(row, "file_base") == "null":
        return "FT_New_Memory_Face null file_base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftsizes.new_size":
        if lifecycle_handle(row, "face") == "null":
            return "FT_New_Size null face validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        if lifecycle_handle(row, "output") == "null":
            return "FT_New_Size null output pointer validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftsizes.done_size" and lifecycle_handle(row, "size") == "null":
        return "FT_Done_Size null size validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftsizes.activate_size" and lifecycle_handle(row, "size") == "null":
        return "FT_Activate_Size null size validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def otvalid_null_validation_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftotval.open_type_validate":
        if param_is_null(row, "face"):
            return "FT_OpenType_Validate null face validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        if "null_output_indices" in row.params:
            return "FT_OpenType_Validate null output pointers validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftotval.open_type_free":
        if param_is_null(row, "face"):
            return "FT_OpenType_Free null face validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        if param_is_null(row, "table") and has_runtime_asset(row):
            return "FT_OpenType_Free null table validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def otvalid_real_parity_reason(row: ConcreteInput) -> str | None:
    if (
        row.operation == "ftotval.open_type_validate"
        and row.params.get("face") == "valid_without_OPENTYPE_VALIDATE_service"
        and has_runtime_asset(row)
    ):
        return "FT_OpenType_Validate missing-service face validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    otvalid_table_success_cases = {
        "ftotval.FT_VALIDATE_BASE.validate_selects_base_table",
        "ftotval.FT_VALIDATE_GDEF.validate_selects_gdef_table",
        "ftotval.FT_VALIDATE_GPOS.validate_selects_gpos_table",
        "ftotval.FT_VALIDATE_GSUB.validate_selects_gsub_table",
        "ftotval.FT_VALIDATE_JSTF.validate_selects_jstf_table",
        "ftotval.FT_VALIDATE_MATH.validate_selects_math_table",
        "ftotval.FT_VALIDATE_OT.validate_all_requested_tables",
    }
    if row.operation == "ftotval.open_type_validate" and row.case_id in otvalid_table_success_cases:
        return "FT_OpenType_Validate table-selection success validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def font_format_real_parity_reason(row: ConcreteInput) -> str | None:
    if row.operation in {"ftfntfmt.get_font_format", "ftfntfmt.get_x11_font_format"}:
        if has_runtime_asset(row) or row.params.get("face") is None:
            return "font-format service string/nullness validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def outline_get_bitmap_real_parity_reason(row: ConcreteInput) -> str | None:
    ftimage_outline_get_bitmap_cases = {
        ("ftimage.FT_Bitmap", "empty_bitmap_is_valid"),
        ("ftimage.FT_OUTLINE_IGNORE_DROPOUTS", "mono_dropout_behavior"),
        ("ftimage.FT_OUTLINE_INCLUDE_STUBS", "mono_stub_dropout_behavior"),
        ("ftimage.FT_OUTLINE_SMART_DROPOUTS", "mono_smart_dropout_behavior"),
        ("ftimage.FT_RASTER_FLAG_DEFAULT", "default_monochrome_target_path"),
    }
    if (
        row.subject == "ftoutln.FT_Outline_Get_Bitmap"
        and row.operation == "ftoutln.outline_get_bitmap"
    ):
        return "FT_Outline_Get_Bitmap validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.subject == "ftimage.FT_PIXEL_MODE_NONE"
        and row.case == "invalid_render_target_errors"
        and row.operation == "ftoutln.outline_get_bitmap"
    ):
        return "FT_PIXEL_MODE_NONE invalid FT_Outline_Get_Bitmap target validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.subject == "ftimage.FT_PIXEL_MODE_NONE"
        and row.case == "empty_bitmap_state"
        and row.operation == "ftbitmap.bitmap_init_and_empty_render"
    ):
        return "FT_PIXEL_MODE_NONE empty bitmap state validates through the maintained empty-outline FT_Outline_Get_Bitmap route across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        (row.subject, row.case) in ftimage_outline_get_bitmap_cases
        and row.operation == "ftoutln.outline_get_bitmap"
    ):
        return "ftimage FT_Outline_Get_Bitmap behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def set_debug_hook_real_parity_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftmodapi.set_debug_hook" and row.subject in {
        "ftmodapi.FT_Set_Debug_Hook",
        "ftmodapi.FT_DEBUG_HOOK_TRUETYPE",
    }:
        return "FT_Set_Debug_Hook slot mutation/no-op behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def add_default_modules_real_parity_reason(row: ConcreteInput) -> str | None:
    if (
        row.operation == "ftmodapi.add_default_modules"
        and row.subject == "ftmodapi.FT_Add_Default_Modules"
    ):
        return "FT_Add_Default_Modules module-table mutation/no-op behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def inspect_module_flags_real_parity_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftmodapi.inspect_module_flags":
        return "module class font-driver flags validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def get_module_real_parity_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftmodapi.get_module":
        return "FT_Get_Module module nullness and class names validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "freetype.module_interface_probe":
        return "FT_Get_Module_Interface module-interface nullness and requester service availability validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def interpreter_version_property_real_parity_reason(row: ConcreteInput) -> str | None:
    exact_rows = {
        "ftdriver.TT_INTERPRETER_VERSION_35.interpreter_version_property_roundtrip",
        "ftdriver.TT_INTERPRETER_VERSION_38.interpreter_version_property_normalizes_to_40",
        "ftdriver.TT_INTERPRETER_VERSION_40.interpreter_version_property_roundtrip",
    }
    if row.operation == "ftdriver.interpreter_version_property" and row.case_id in exact_rows:
        return "TT interpreter-version property set/get validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def focused_success_real_parity_reason(row: ConcreteInput) -> str | None:
    if (
        row.operation == "ftoutln.outline_render_direct"
        and row.case_id == "ftimage.FT_Raster_Span_Func.direct_render_emits_spans"
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Outline_Render direct span callback output validates emitted "
            "span y/x/len/coverage tuples, observed clipping, user callback "
            "identity, and target preservation through pinned C oracle, Rust "
            "FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftsystem.memory_stream_probe"
        and row.case_id == "ftsystem.FT_StreamRec.memory_stream_field_contract"
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_StreamRec memory-stream field contract validates "
            "FT_New_Memory_Face stream base/size/pos/cursor/limit nullness and "
            "declared frame byte reads through pinned C oracle, Rust FFI, C ABI, "
            "and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_private_mm_blend"
        and row.case_id != "t1tables.T1_BLEND_FORCE_BOLD.private_force_bold_runtime_value"
        and "rows" not in row.params
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_PS_Font_Private Type1 private dictionary fields validate "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_private_mm_blend"
        and row.case_id == "t1tables.T1_BLEND_FORCE_BOLD.private_force_bold_runtime_value"
        and "rows" in row.params
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_PS_Font_Private Type1 ForceBold MM/non-MM rows validate "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if row.operation in {
        "t1tables.get_ps_font_info_mm_blend",
        "t1tables.t1_blend_flags_font_info_group",
    } and unresolved_assets_reason(row) is None:
        return (
            "FT_Get_PS_Font_Info Type1 FontInfo fields validate through pinned "
            "C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_private"
        and row.case_id
        in {
            "t1tables.FT_Get_PS_Font_Private.type1_font_value_populated_success",
            "t1tables.FT_Get_PS_Font_Private.cff_invalid_argument",
        }
        and unresolved_assets_reason(row) is None
    ):
        if row.expect_error:
            return (
                "FT_Get_PS_Font_Private CFF unsupported-service Invalid_Argument validates "
                "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
            )
        return (
            "FT_Get_PS_Font_Private Type1 private dictionary record validates "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_info"
        and row.case_id
        in {
            "t1tables.FT_Get_PS_Font_Info.type1_font_value_populated_success",
            "t1tables.FT_Get_PS_Font_Info.cff_fontinfo_populated_success",
            "t1tables.FT_Get_PS_Font_Info.truetype_invalid_argument",
            "t1tables.FT_Get_PS_Font_Info.null_face_invalid_face_handle",
            "t1tables.FT_Get_PS_Font_Info.null_output_invalid_argument",
        }
        and unresolved_assets_reason(row) is None
    ):
        if row.expect_error:
            if row.case_id == "t1tables.FT_Get_PS_Font_Info.truetype_invalid_argument":
                return (
                    "FT_Get_PS_Font_Info TrueType unsupported-service Invalid_Argument "
                    "validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
                )
            return (
                "FT_Get_PS_Font_Info null face/output error behavior validates "
                "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
            )
        if row.case_id == "t1tables.FT_Get_PS_Font_Info.cff_fontinfo_populated_success":
            return (
                "FT_Get_PS_Font_Info CFF top-dict FontInfo string/scalar record "
                "validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
            )
        return (
            "FT_Get_PS_Font_Info Type1 FontInfo string/scalar record validates "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.has_ps_glyph_names"
        and row.case_id
        in {
            "t1tables.FT_Has_PS_Glyph_Names.type1_font_value_populated_true",
            "t1tables.FT_Has_PS_Glyph_Names.cff_fontinfo_populated_true",
            "t1tables.FT_Has_PS_Glyph_Names.truetype_false",
            "t1tables.FT_Has_PS_Glyph_Names.cid_keyed_cff_false",
            "t1tables.FT_Has_PS_Glyph_Names.null_face_false",
        }
        and unresolved_assets_reason(row) is None
    ):
        if row.case_id == "t1tables.FT_Has_PS_Glyph_Names.cff_fontinfo_populated_true":
            return (
                "FT_Has_PS_Glyph_Names CFF glyph-name flag behavior validates "
                "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
            )
        if row.case_id == "t1tables.FT_Has_PS_Glyph_Names.cid_keyed_cff_false":
            return (
                "FT_Has_PS_Glyph_Names CID-keyed CFF false result validates "
                "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
            )
        return (
            "FT_Has_PS_Glyph_Names Type1, TrueType, and null-face results "
            "validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_private"
        and row.case_id
        == "t1tables.FT_Get_PS_Font_Private.truetype_invalid_argument"
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_PS_Font_Private TrueType unsupported-service Invalid_Argument "
            "validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_private"
        and row.case_id
        in {
            "t1tables.FT_Get_PS_Font_Private.null_face_invalid_face_handle",
            "t1tables.FT_Get_PS_Font_Private.null_output_invalid_argument",
        }
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_PS_Font_Private null face/output error behavior validates "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.t1_blend_flags_private_group"
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_PS_Font_Private Type1 blend private group validates through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.mm_blend_dictionary"
        and row.case_id
        in {
            "t1tables.T1_BLEND_UNDERLINE_POSITION.blend_dictionary_runtime",
            "t1tables.T1_BLEND_UNDERLINE_THICKNESS.blend_dictionary_runtime",
        }
        and unresolved_assets_reason(row) is None
    ):
        return (
            "Type1 MM FontInfo underline blend dictionary presence and public "
            "FT_Get_PS_Font_Info field output validate through pinned C oracle, "
            "Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftbdf.get_bdf_property"
        and row.case_id
        == "ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties"
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_BDF_Property BDF property rowset validates actual pinned C "
            "FAMILY_NAME missing-property behavior and POINT_SIZE/PIXEL_SIZE "
            "integer outputs through Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.sbit_cache_lookup_scaler"
        and row.case_id
        in {
            "ftcache.FTC_SBitCache_LookupScaler.scaler_size_semantics_match_c",
            "ftcache.FTC_SBitCache_LookupScaler.load_flags_truncate_to_int32",
        }
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FTC_SBitCache_LookupScaler scaler size-selection and FT_ULong "
            "load-flag truncation outputs validate through pinned C oracle, "
            "Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "freetype.open_face_args"
        and row.case_id
        in {
            "freetype.FT_Open_Args.memory_source_success_matches_c",
            "freetype.FT_Open_Args.memory_source_error_variants_match_c",
            "freetype.FT_Open_Args.memory_source_negative_face_index_probe_matches_c",
            "freetype.FT_Open_Args.memory_source_out_of_range_face_index_matches_c",
            "freetype.FT_Open_Args.memory_source_short_sizes_match_c",
            "freetype.FT_Open_Args.memory_source_truncated_sfnt_size_matches_c",
            "freetype.FT_Open_Args.memory_source_optional_flags_noop_match_c",
            "freetype.FT_Open_Args.source_flag_error_matrix_matches_c",
        }
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Open_Args FT_OPEN_MEMORY source dispatch, invalid source flags, "
            "null args/library/aface rows validate exact "
            "FT_Open_Face status plus output pointer nullness through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_value"
        and row.case_id == "t1tables.FT_Get_PS_Font_Value.signature_and_behavior_matrix"
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_PS_Font_Value selector matrix validates scalar, string, "
            "array, encoding, sizing-query, short-buffer, negative-length, "
            "unsupported-service, non-PostScript, and null-face behavior "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_value"
        and row.case_id.startswith("t1tables.T1_ENCODING_TYPE_")
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_PS_Font_Value PS_DICT_ENCODING_TYPE validates Type1 "
            "encoding classification through pinned C oracle, Rust FFI, C ABI, "
            "and WASM ABI"
        )
    if (
        row.operation == "t1tables.get_ps_font_value_encoding"
        and row.case_id
        in {
            "t1tables.T1_EncodingType.array_encoding_runtime_case",
            "t1tables.T1_EncodingType.standard_or_expert_runtime_cases",
        }
        and unresolved_assets_reason(row) is None
    ):
        return (
            "FT_Get_PS_Font_Value Type1 encoding type and entry rows validate "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    case_reasons = {
        "freetype.FT_Bitmap_Size.available_sizes_values_match_c": "FT_Bitmap_Size available_sizes validates WinFNT fixed-size public records through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_FaceRec.available_sizes_public_fields_match_c": "FT_FaceRec num_fixed_sizes and available_sizes pointer-derived records validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_FaceRec.charmap_public_fields_match_c": "FT_FaceRec num_charmaps, owned charmap records, active charmap selection, and char-index probes validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_Parameter.typographic_name_params_match_c": "FT_Parameter typographic family/subfamily tag dispatch validates through FT_Open_Face with pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_Parameter.ignored_open_params_match_c": "FT_Parameter ignored open-face tags validate no-effect public output through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_Parameter.incremental_null_data_matches_c": "FT_Parameter incremental absent/null-data dispatch validates embedded glyph loading and null callback state through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
    }
    return case_reasons.get(row.case_id)


def done_mm_var_real_parity_reason(row: ConcreteInput) -> str | None:
    if (
        row.operation == "ftmm.done_mm_var"
        and row.subject == "ftmm.FT_Done_MM_Var"
    ):
        return "FT_Done_MM_Var library/descriptor ownership behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation in {"ftmm.get_mm_var_then_done", "ftmm.get_and_done_mm_var"}
        and row.case_id
        in {
            "ftmm.FT_Done_MM_Var.frees_descriptor_success",
            "ftmm.FT_MM_Var.ownership_matches_c",
        }
    ):
        return "FT_Get_MM_Var descriptor acquisition followed by FT_Done_MM_Var release validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def future_batch_unresolved_asset_pending_reason(row: ConcreteInput) -> str | None:
    if row.operation not in (
        FTMM_SUCCESS_OPERATIONS
        | FTDRIVER_SUCCESS_OPERATIONS
        | FTMODAPI_SUCCESS_OPERATIONS
    ):
        return None
    if row.expectation_status not in {"ok", "build_dependent"}:
        return None
    unresolved = unresolved_assets_reason(row)
    if not unresolved:
        return None
    return (
        f"{unresolved}; future-batch success output cannot count as real "
        "parity until the fixture is C-openable and the Rust FFI, C ABI, and "
        "WASM ABI routes compare exact output"
    )


def future_batch_real_parity_reason(row: ConcreteInput) -> str | None:
    face_properties_rows = {
        "freetype.FT_Face_Properties.success_supported_face_properties",
        "freetype.FT_Face_Properties.success_zero_properties_noop",
        "freetype.FT_Face_Properties.error_invalid_property_tag_or_value",
        "ftparams.FT_PARAM_TAG_LCD_FILTER_WEIGHTS.malformed_data_does_not_read_as_weights",
        "ftparams.FT_PARAM_TAG_RANDOM_SEED.null_or_wrong_size_errors",
        "ftparams.FT_PARAM_TAG_STEM_DARKENING.unsupported_or_null_data_matches_c_error",
    }
    if row.operation == "freetype.face_properties" and row.case_id in face_properties_rows:
        return "FT_Face_Properties scalar tags validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.glyph_slot_reuse"
        and row.case_id == "freetype.FT_GlyphSlot.overwritten_by_subsequent_load"
    ):
        return "FT_GlyphSlot load-sequence overwrite output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (row.operation, row.case_id) in FTERRDEF_EXACT_ERROR_BATCH:
        return "fterrdef load-glyph exact error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation in FTMM_SUCCESS_OPERATIONS
        and row.expectation_status == "ok"
        and has_runtime_asset(row)
        and unresolved_assets_reason(row) is None
        and ftmm_subsystem_pending_reason(row) is None
    ):
        return "FT multiple-master/variation success output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_blend_coordinates"
        and row.case_id == "ftmm.FT_Get_MM_Blend_Coordinates.default_blend_coordinates"
        and row.expectation_status == "ok"
        and unresolved_asset_reason(row.assets.get("variable_font"), "variable_font") is None
        and ftmm_subsystem_pending_reason(row) is None
    ):
        return "FT_Get_MM_Blend_Coordinates default variable-font blend output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_blend_coordinates"
        and row.case_id == "ftmm.FT_Get_MM_Blend_Coordinates.partial_or_excess_count"
        and row.expectation_status == "ok"
        and unresolved_asset_reason(row.assets.get("variable_font"), "variable_font") is None
        and ftmm_subsystem_pending_reason(row) is None
    ):
        return "FT_Get_MM_Blend_Coordinates variable-font count matrix validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation in FTDRIVER_SUCCESS_OPERATIONS
        and row.expectation_status in {"ok", "build_dependent"}
        and has_runtime_asset(row)
        and unresolved_assets_reason(row) is None
    ):
        return "FT driver property/interpreter success output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation in FTMODAPI_SUCCESS_OPERATIONS
        and row.expectation_status in {"ok", "build_dependent"}
        and has_runtime_asset(row)
        and unresolved_assets_reason(row) is None
    ):
        return "FT module API runtime success output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation in SFNT_METADATA_SUCCESS_OPERATIONS
        and row.expectation_status in {"ok", "build_dependent"}
        and has_runtime_asset(row)
    ):
        return "SFNT/charmap metadata runtime output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation in FTRENDER_SUCCESS_OPERATIONS
        and row.expectation_status in {"ok", "build_dependent"}
    ):
        return "FT renderer selection output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation in FTLOGGING_SUCCESS_OPERATIONS
        and row.expectation_status in {"ok", "build_dependent"}
    ):
        return "FT debug logging output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    case_reasons = {
        "ftdriver.FT_AUTOHINTER_SCRIPT_CJK.fallback_script_property_roundtrip": "FT_Property_Set/Get autofitter fallback-script CJK scalar roundtrip validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_AUTOHINTER_SCRIPT_CJK.glyph_to_script_map_runtime": "FT_Prop_GlyphToScriptMap CJK runtime map entry and force-autohint glyph output validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_AUTOHINTER_SCRIPT_INDIC.fallback_script_property_validation": "FT_Property_Set/Get autofitter fallback-script Indic scalar validation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_AUTOHINTER_SCRIPT_INDIC.glyph_to_script_map_runtime": "FT_Prop_GlyphToScriptMap Indic runtime map entry and force-autohint glyph output validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_AUTOHINTER_SCRIPT_LATIN.default_script_property_roundtrip": "FT_Property_Set/Get autofitter default-script Latin scalar roundtrip validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_AUTOHINTER_SCRIPT_NONE.default_and_fallback_property_roundtrip": "FT_Property_Set/Get autofitter NONE default/fallback scalar roundtrip plus typed glyph-to-script-map output validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_AUTOHINTER_SCRIPT_LATIN.glyph_to_script_map_runtime": "FT_Prop_GlyphToScriptMap Latin runtime map entry and force-autohint glyph output validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_AUTOHINTER_SCRIPT_NONE.glyph_to_script_map_runtime": "FT_Prop_GlyphToScriptMap NONE runtime map entry and force-autohint glyph output validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_From_Glyph_Index.cid_face_returns_cid": "CID glyph-index output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_From_Glyph_Index.opentype_cid_face_supported": "CID glyph-index output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_From_Glyph_Index.opentype_cid_null_output_ok": "CID glyph-index null output behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_From_Glyph_Index.null_cid_output_matches_c": "CID glyph-index output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_Is_Internally_CID_Keyed.cid_face_reports_true": "CID-keyed face output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_Is_Internally_CID_Keyed.sfnt_wrapped_cid_supported": "CID-keyed face output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_Is_Internally_CID_Keyed.sfnt_wrapped_cid_null_output_ok": "CID-keyed null output behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_Is_Internally_CID_Keyed.null_output_matches_c": "CID-keyed face output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftcid.FT_Get_CID_Registry_Ordering_Supplement.success_cid_keyed_face": "CID registry/ordering/supplement output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_SizeRec.active_size_record_runtime": "FT_SizeRec public face/generic/metrics/internal state validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftglyph.FT_New_Glyph.success_bitmap_outline_svg_empty_glyph": "FT_New_Glyph supported empty glyph allocation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_Prop_GlyphToScriptMap.property_get_returns_face_map": "FT_Property_Get glyph-to-script-map output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.FT_Prop_IncreaseXHeight.property_set_get_round_trips_limit": "FT_Property_Set/Get increase-x-height face-scoped limit roundtrip validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "fterrdef.FT_Err_Missing_Property.known_property_success": "FT_Property_Get known autofitter fallback-script property success validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftdriver.TT_INTERPRETER_VERSION_40.default_interpreter_version": "FT_Property_Get TrueType interpreter-version default validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftmodapi.FT_Done_Library.default_modules_final_destroy_status": "FT_Done_Library final destroy status for a default-module library validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI; owned face closure and synthetic module destructor ordering remain pending",
        "ftgxval.FT_VALIDATE_GX.validates_all_requested_tables": "FT_TrueTypeGX_Validate table-selection output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_GX_LENGTH.controls_output_slot_initialization": "FT_TrueTypeGX_Validate output-slot initialization validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_MS.validates_ms_classic_kern": "FT_ClassicKern_Validate MS/classic-kern output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_bsln.validates_bsln_table_slot": "FT_TrueTypeGX_Validate bsln table slot validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_bsln_INDEX.indexes_bsln_output_slot": "FT_TrueTypeGX_Validate bsln output index validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_feat.validates_feat_table_slot": "FT_TrueTypeGX_Validate feat table slot validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_feat_INDEX.indexes_feat_output_slot": "FT_TrueTypeGX_Validate feat output index validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftstroke.FT_Stroker_New.valid_library_allocates_stroker": "FT_Stroker_New non-null allocation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftstroke.FT_Stroker_Done.valid_stroker_releases_buffers": "FT_Stroker_Done non-null release validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftstroke.FT_Stroker_Done.after_export_cleanup": "FT_Stroker_Done after-export ownership validates caller-owned outline preservation through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftstroke.FT_Stroker.unparsed_handle_lifecycle_matches_c": "FT_Stroker unparsed non-null handle lifecycle validates New, Set, unparsed Export/ExportBorder no-op, Rewind, and Done through pinned C oracle, Rust FFI, C ABI, and WASM ABI; path geometry/count lifecycle remains pending",
        "ftstroke.FT_Stroker.lifecycle_contract": "FT_Stroker parsed lifecycle validates New, Set, BeginSubPath, two LineTo calls, EndSubPath, GetCounts, Export, and Done status/count behavior through pinned C oracle, Rust FFI, C ABI, and WASM ABI; exported join geometry remains pending elsewhere",
        "ftstroke.FT_Stroker_Export.invalid_inputs_noop": "FT_Stroker_Export null/invalid-input no-op validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftstroke.FT_Stroker_Export.append_to_existing_outline": "FT_Stroker_Export append-to-existing-outline behavior validates sentinel preservation plus point/tag/contour offset appends through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftstroke.FT_Stroker_ExportBorder.invalid_inputs_or_border_noop": "FT_Stroker_ExportBorder null/invalid-border/unparsed-stroker no-op validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftstroke.FT_Stroker_ExportBorder.append_to_existing_outline": "FT_Stroker_ExportBorder append-to-existing-outline behavior validates sentinel preservation plus selected-border point/tag/contour offset appends through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftrender.FT_Get_Renderer.outline_renderer_lookup_success": "FT_Get_Renderer outline renderer class metadata validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftrender.FT_Get_Renderer.bitmap_svg_and_unknown_formats": "FT_Get_Renderer bitmap/SVG/outline/unknown renderer class metadata validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftrender.FT_Get_Renderer.null_library_returns_null": "FT_Get_Renderer null-library lookup returns no renderer through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_just.validates_just_table_slot": "FT_TrueTypeGX_Validate just table slot validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_just_INDEX.indexes_just_output_slot": "FT_TrueTypeGX_Validate just output index validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_kern.validates_gx_kern_table_slot": "FT_TrueTypeGX_Validate kern table slot validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_kern_INDEX.indexes_kern_output_slot": "FT_TrueTypeGX_Validate kern output index validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_lcar.validates_lcar_table_slot": "FT_TrueTypeGX_Validate lcar table slot validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_lcar_INDEX.indexes_lcar_output_slot": "FT_TrueTypeGX_Validate lcar output index validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_mort.validates_mort_table_slot": "FT_TrueTypeGX_Validate mort table slot validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_mort_INDEX.indexes_mort_output_slot": "FT_TrueTypeGX_Validate mort output index validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_morx.validates_morx_table_slot": "FT_TrueTypeGX_Validate morx table slot validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftgxval.FT_VALIDATE_morx_INDEX.indexes_morx_output_slot": "FT_TrueTypeGX_Validate morx output index validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftimage.FT_GLYPH_FORMAT_NONE.reset_slot_uses_none": "FT_GLYPH_FORMAT_NONE new-face and failed-load slot state validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftmodapi.FT_DEBUG_HOOK_TRUETYPE.debug_hook_index_import_contract": "FT_Set_Debug_Hook TrueType hook index contract validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftmodapi.FT_Set_Default_Properties.no_environment_noop": "FT_Set_Default_Properties unset-environment behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftmodapi.FT_Set_Default_Properties.parses_supported_environment_property": "FT_Set_Default_Properties environment handling validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftmodapi.FT_Set_Default_Properties.ignores_malformed_or_failed_properties": "FT_Set_Default_Properties malformed, missing-property, and null-library ignored-error behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftpfr.FT_Get_PFR_Advance.pfr_glyph_advance_success": "PFR advance output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftpfr.FT_Get_PFR_Kerning.pfr_pair_kerning_success": "PFR kerning output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftpfr.FT_Get_PFR_Kerning.non_pfr_falls_back_to_unscaled_kerning": "FT_Get_PFR_Kerning non-PFR fallback validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
    }
    return case_reasons.get(row.case_id)


def lifecycle_null_real_parity_reason(row: ConcreteInput) -> str | None:
    exact_error_reasons = {
        (
            "FT_New_Face",
            "fterrdef.FT_Err_Cannot_Open_Resource.missing_path_returns_error",
        ): "FT_New_Face missing-path error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_New_Face",
            "fterrdef.FT_Err_Cannot_Open_Stream.zero_length_file_returns_error",
        ): "FT_New_Face zero-length-file error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_New_Open_Face",
            "fterrdef.FT_Err_Invalid_Stream_Operation.stream_operation_failure",
        ): "FT_New_Open_Face stream-operation failure validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_New_Open_Face",
            "fterrdef.FT_Err_Invalid_Stream_Seek.stream_seek_failure",
        ): "FT_New_Open_Face stream-seek failure validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Request_Size",
            "fterrdef.FT_Err_Invalid_Pixel_Size.bitmap_strike_mismatch",
        ): "FT_Request_Size bitmap-strike mismatch error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Set_Char_Size_then_FT_Load_Glyph",
            "fterrdef.FT_Err_Invalid_PPem.tt_size_reset_zero_ppem",
        ): "FT_Set_Char_Size then FT_Load_Glyph zero-ppem error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Stream_OpenBzip2",
            "fterrdef.FT_Err_Unimplemented_Feature.optional_module_feature_disabled",
        ): "FT_Stream_OpenBzip2 disabled optional-module error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_New_Memory_Face_or_FT_New_Open_Face",
            "fterrdef.FT_Err_Invalid_Stream_Skip.pcf_table_skip_failure",
        ): "PCF table stream-skip failure validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_New_Memory_Face_and_table_public_api",
            "fterrdef.FT_Err_Invalid_Table.malformed_sfnt_table_rejected",
        ): "FT_New_Memory_Face malformed SFNT table error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_New_Memory_Face_then_charmap_ops",
            "fterrdef.FT_Err_CMap_Table_Missing.sfnt_without_cmap_returns_error_where_required",
        ): "SFNT missing-cmap charmap operation error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Get_Glyph_or_FT_Render_Glyph",
            "fterrdef.FT_Err_Invalid_Slot_Handle.null_or_invalid_slot_rejected",
        ): "FT_Get_Glyph/Render_Glyph null-or-invalid slot error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Load_Glyph_and_FT_Render_Glyph",
            "fterrdef.FT_Err_Invalid_Outline.malformed_outline_rejected",
        ): "FT_Load_Glyph/Render_Glyph malformed outline error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Load_Glyph_then_FT_Render_Glyph",
            "fterrdef.FT_Err_Cannot_Render_Glyph.unsupported_render_mode_returns_error",
        ): "FT_Render_Glyph unsupported render-mode error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Property_Get",
            "ftdriver.FT_Prop_GlyphToScriptMap.invalid_face_error_matches_c",
        ): "FT_Property_Get glyph-to-script-map invalid-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Property_Get",
            "fterrdef.FT_Err_Missing_Property.driver_property_unknown_name",
        ): "FT_Property_Get unknown driver property error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Property_Set_or_Get",
            "ftdriver.FT_Prop_IncreaseXHeight.invalid_face_error_matches_c",
        ): "FT_Property_Get/Set increase-x-height invalid-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "allocation_failure_harness.array_growth",
            "fterrdef.FT_Err_Array_Too_Large.allocator_growth_overflow_returns_error",
        ): "allocator array-growth overflow error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "allocation_failure_harness.fail_after_n",
            "fterrdef.FT_Err_Out_Of_Memory.allocator_failure_injection",
        ): "allocator fail-after-N out-of-memory error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftbitmap.glyphslot_own_bitmap",
            "ftbitmap.FT_GlyphSlot_Own_Bitmap.error_copy_allocation_failure",
        ): "FT_GlyphSlot_Own_Bitmap bitmap-copy allocation failure validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftmm.set_named_instance",
            "ftmm.FT_Var_Named_Style.selected_instance_matches_descriptor",
        ): "FT_Set_Named_Instance selected design coordinates validate against FT_Var_Named_Style fvar descriptor coordinates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FTC_cache_lookup",
            "fterrdef.FT_Err_Out_Of_Memory.cache_flush_then_oom",
        ): "FTC cache flush-then-OOM error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ffi_error_mapping",
            "fterrdef.FT_Err_Invalid_Outline.rust_invalid_outline_mapping",
        ): "FFI invalid-outline error mapping validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "face.new",
            "fterrdef.FT_Err_Invalid_Post_Table_Format.sfnt_post_format_rejected",
        ): "FT_New_Face invalid-post-format exact error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "face.new",
            "tttables.TT_Postscript.invalid_post_format_error_runtime",
        ): "TT_Postscript invalid-post-format runtime error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "freetype.face_properties_then_render",
            "ftparams.FT_PARAM_TAG_LCD_FILTER_WEIGHTS.face_property_ignored",
        ): "FT_Face_Properties LCD filter weights ignored behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.done_glyph",
            "ftglyph.FT_Done_Glyph.success_null_is_noop",
        ): "FT_Done_Glyph(NULL) void no-op validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.done_glyph",
            "ftglyph.FT_Done_Glyph.success_releases_owned_outline_glyph",
        ): "FT_Done_Glyph outline ownership validates a real FT_Get_Glyph outline, copied outline arrays before release, and one public release call through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.done_glyph",
            "ftglyph.FT_Done_Glyph.success_releases_owned_bitmap_glyph",
        ): "FT_Done_Glyph bitmap ownership validates a real FT_Get_Glyph bitmap, owned bitmap buffer fields before release, and one public release call through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.done_glyph",
            "ftglyph.FT_OutlineGlyphRec.owns_outline_arrays",
        ): "FT_Done_Glyph outline-glyph ownership validates a real FT_Get_Glyph outline, owned outline flags/counts before release, and one public release call through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.done_glyph",
            "ftglyph.FT_Done_Glyph.outline_glyph_before_library_done",
        ): "FT_Done_Glyph outline glyph-before-library lifetime validates a real FT_Get_Glyph outline is released before face/library teardown through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.done_glyph",
            "ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer_get_glyph_bitmap",
        ): "FT_Done_Glyph bitmap-glyph ownership validates a real FT_Get_Glyph bitmap, owned bitmap buffer fields before release, and one public release call through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.done_glyph",
            "ftglyph.FT_BitmapGlyphRec.owns_bitmap_buffer",
        ): "FT_BitmapGlyphRec ownership validates both FT_Get_Glyph bitmap and FT_Glyph_To_Bitmap outline bitmap-glyph creation paths, owned bitmap buffer fields before release, and one public FT_Done_Glyph release per owned bitmap glyph through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.record_inspect",
            "ftglyph.FT_BitmapGlyphRec.fields_match_get_glyph_and_to_bitmap",
        ): "FT_BitmapGlyphRec fields validate both FT_Get_Glyph bitmap and FT_Glyph_To_Bitmap outline creation paths through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.type_runtime",
            "ftglyph.FT_Glyph.outline_caller_owned_lifetime",
        ): "FT_Glyph outline caller-owned lifetime validates a real FT_Get_Glyph outline record and release through public behavior against pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.type_runtime",
            "ftglyph.FT_Glyph.bitmap_caller_owned_lifetime",
        ): "FT_Glyph bitmap caller-owned lifetime validates a real FT_Get_Glyph bitmap record and release through public behavior against pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.type_runtime",
            "ftglyph.FT_Glyph_Class.outline_class_behavior",
        ): "FT_Glyph_Class outline behavior validates the private class only through public FT_Get_Glyph outline output against pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftglyph.type_runtime",
            "ftglyph.FT_Glyph_Class.bitmap_class_behavior",
        ): "FT_Glyph_Class bitmap behavior validates the private class only through public FT_Get_Glyph bitmap output against pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftoutln.outline_copy",
            "ftoutln.FT_Outline_Copy.invalid_pointer_or_size_mismatch",
        ): "FT_Outline_Copy invalid-pointer/size mismatch error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftoutln.outline_done",
            "ftoutln.FT_Outline_Done.invalid_library_or_outline_errors",
        ): "FT_Outline_Done invalid-library/outline errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftoutln.outline_embolden",
            "ftoutln.FT_Outline_Embolden.invalid_or_indeterminate_orientation_errors",
        ): "FT_Outline_Embolden invalid/indeterminate-orientation errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftoutln.outline_embolden_xy",
            "ftoutln.FT_Outline_EmboldenXY.invalid_orientation_or_null_errors",
        ): "FT_Outline_EmboldenXY invalid-orientation/null errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftoutln.outline_new",
            "ftoutln.FT_Outline_New.invalid_arguments_and_limits",
        ): "FT_Outline_New invalid-argument/limit errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftoutln.outline_reverse_orientation",
            "ftoutln.FT_ORIENTATION_FILL_LEFT.reverse_toggles_orientation_fixture",
        ): "FT_Outline_Reverse followed by orientation, cbox, bbox, decompose, and bitmap observation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "freetype.new_size",
            "fterrdef.FT_Err_Invalid_Driver_Handle.module_driver_handle_validation",
        ): "FT_New_Size invalid-driver-handle error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "glyphdict.name_index_lookup",
            "fterrdef.FT_Err_Invalid_Character_Code.char_index_name_lookup_invalid_code",
        ): "glyph name-index lookup invalid-character-code error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftimage.custom_renderer_lifecycle",
            "ftimage.FT_Raster_New_Func.renderer_new_error_propagates",
        ): "FT_Raster_New custom renderer error propagation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftincrem.load_incremental_glyph",
            "ftincrem.FT_Incremental_FuncsRec.callback_error_propagates",
        ): "FT_Incremental glyph callback error propagation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftoutln.outline_geometry",
            "ftoutln.FT_Orientation.geometry_fixture_matrix",
        ): "FT_Outline orientation geometry fixture errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftoutln.outline_render_direct",
            "ftimage.FT_Raster_Span_Func.missing_span_callback_errors",
        ): "FT_Outline_Render direct missing-span-callback error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "raster.module_error_probe",
            "ftmoderr.FT_Mod_Err_Raster.prefixed_error_base",
        ): "Raster module prefixed error base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "sdf.render_error_probe",
            "ftmoderr.FT_Mod_Err_Sdf.prefixed_error_base",
        ): "SDF module prefixed error base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "sfnt.load_glyph_or_color_table",
            "fterrdef.FT_Err_Invalid_Offset.table_offset_bounds_rejected",
        ): "SFNT glyph/color table invalid-offset error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "sfnt.load_glyph_or_metrics",
            "fterrdef.FT_Err_Invalid_Horiz_Metrics.sfnt_hmtx_metrics_rejected",
        ): "SFNT horizontal metrics rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "sfnt.load_sfnt_table",
            "fterrdef.FT_Err_Table_Missing.sfnt_required_table_missing",
        ): "FT_Load_Sfnt_Table required-table-missing error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "sfnt.new_memory_face_then_charmap_lookup",
            "fterrdef.FT_Err_Invalid_CharMap_Format.sfnt_cmap_format_rejected",
        ): "SFNT charmap format rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "sfnt.face_load_error_probe",
            "ftmoderr.FT_Mod_Err_SFNT.prefixed_error_base",
        ): "SFNT module prefixed error base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "truetype.load_glyph_error_probe",
            "ftmoderr.FT_Mod_Err_TrueType.prefixed_error_base",
        ): "TrueType module prefixed error base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "type1.load_glyph_error_probe",
            "ftmoderr.FT_Mod_Err_Type1.prefixed_error_base",
        ): "Type1 module prefixed error base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "type42.face_load_error_probe",
            "ftmoderr.FT_Mod_Err_Type42.prefixed_error_base",
        ): "Type42 module prefixed error base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "winfonts.face_load_error_probe",
            "ftmoderr.FT_Mod_Err_Winfonts.prefixed_error_base",
        ): "WinFNT module prefixed error base validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "svg.preset_slot",
            "fterrdef.FT_Err_Bad_Argument.svg_preset_slot_bad_argument",
        ): "SVG preset-slot bad-argument error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftlcdfil.set_lcd_filter",
            "ftlcdfil.FT_Library_SetLcdFilter.unimplemented_without_subpixel_filtering",
        ): "FT_Library_SetLcdFilter unavailable-subpixel error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftlcdfil.set_lcd_filter_weights",
            "ftlcdfil.FT_Library_SetLcdFilterWeights.unimplemented_without_subpixel_filtering",
        ): "FT_Library_SetLcdFilterWeights unavailable-subpixel error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftmm.get_multi_master",
            "ftmm.FT_Get_Multi_Master.null_output_error",
        ): "FT_Get_Multi_Master null-output error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "FT_Stream_Seek_or_FT_Stream_Read",
            "fterrdef.FT_Err_Invalid_Stream_Handle.null_stream_rejected",
        ): "FT_Stream_Seek/Read null-stream error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftstream.frame_lifecycle",
            "fterrdef.FT_Err_Invalid_Frame_Operation.stream_frame_access_rejects_invalid_sequence",
        ): "FT_Stream frame lifecycle invalid-sequence error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "stream_frame_harness.enter_twice",
            "fterrdef.FT_Err_Nested_Frame_Access.stream_nested_frame_guard",
        ): "FT_Stream nested-frame guard error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftmm.get_mm_var_then_axis_flags",
            "ftmm.FT_Get_Var_Axis_Flags.out_of_range_axis_error",
        ): "FT_Get_Var_Axis_Flags out-of-range axis error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftmm.get_var_axis_flags",
            "ftmm.FT_Get_Var_Axis_Flags.null_master_or_flags_error",
        ): "FT_Get_Var_Axis_Flags null-master/null-flags error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "ftsnames.get_sfnt_lang_tag",
            "ftsnames.FT_Get_Sfnt_LangTag.format0_invalid_table_error",
        ): "FT_Get_Sfnt_LangTag format-0 invalid-table error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        (
            "sfnt.table_info",
            "tttables.FT_Sfnt_Table_Info.invalid_face_error",
        ): "FT_Sfnt_Table_Info invalid-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
    }
    reason = exact_error_reasons.get((row.operation, row.case_id))
    if reason:
        return reason
    if (
        row.operation == "freetype.init_free_type"
        and row.case_id == "freetype.FT_Init_FreeType.error_null_output_pointer"
    ):
        return "FT_Init_FreeType null-output-pointer error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.get_track_kerning"
        and row.case_id == "freetype.FT_Get_Track_Kerning.error_null_face_or_output"
    ):
        return "FT_Get_Track_Kerning null-face/null-output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.get_track_kerning"
        and row.case_id == "freetype.FT_Get_Track_Kerning.sfnt_or_no_track_data_error"
    ):
        return "FT_Get_Track_Kerning SFNT/no-track-data error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.attach_file"
        and row.case_id == "freetype.FT_Attach_File.error_null_face"
    ):
        return "FT_Attach_File null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.attach_file"
        and row.case_id == "freetype.FT_Attach_File.error_null_pathname"
    ):
        return "FT_Attach_File null-pathname error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.attach_file"
        and row.case_id == "freetype.FT_Attach_File.error_missing_or_unsupported_file"
    ):
        return "FT_Attach_File missing-or-unsupported-file error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.attach_stream"
        and row.case_id == "freetype.FT_Attach_Stream.error_null_face"
    ):
        return "FT_Attach_Stream null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.attach_stream"
        and row.case_id == "freetype.FT_Attach_Stream.error_null_open_args"
    ):
        return "FT_Attach_Stream null-open-args error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.attach_stream"
        and row.case_id
        == "freetype.FT_Attach_Stream.error_invalid_open_args_or_unsupported_driver"
    ):
        return "FT_Attach_Stream invalid-open-args/unsupported-driver error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "new_memory_face"
        and row.case_id == "freetype.FT_New_Memory_Face.error_null_file_base"
    ):
        return "FT_New_Memory_Face null file_base errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "new_memory_face"
        and row.case_id == "freetype.FT_New_Memory_Face.error_null_library_or_aface"
    ):
        return "FT_New_Memory_Face null library/aface errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "new_memory_face"
        and row.case_id == "freetype.FT_New_Memory_Face.error_bad_size_or_unknown_format"
    ):
        return "FT_New_Memory_Face bad-size/unknown-format errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "new_memory_face"
        and row.case_id == "freetype.FT_Open_Face.error_null_library_args_or_aface"
    ):
        return "FT_Open_Face null library/args/aface errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "new_memory_face"
        and row.case_id == "freetype.FT_Open_Face.error_invalid_source_flags"
    ):
        return "FT_Open_Face invalid source-flag errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    face_table_error_cases = {
        (
            "FT_Open_Face",
            "fterrdef.FT_Err_Cannot_Open_Resource.resource_fork_open_failure_returns_error",
        ),
        (
            "FT_Open_Face",
            "fterrdef.FT_Err_Cannot_Open_Stream.resource_fork_stream_failure_returns_error",
        ),
        (
            "FT_Open_Face",
            "fterrdef.FT_Err_Hmtx_Table_Missing.incremental_metrics_exception_matches_c",
        ),
        (
            "new_memory_face",
            "fterrdef.FT_Err_Array_Too_Large.ttc_header_overflow_returns_error",
        ),
        (
            "new_memory_face",
            "fterrdef.FT_Err_Hmtx_Table_Missing.sfnt_missing_hmtx_returns_error",
        ),
        ("FT_Open_Face", "fterrdef.FT_Err_Missing_Module.no_driver_matches_face"),
        (
            "FT_Open_Face",
            "fterrdef.FT_Err_Unknown_File_Format.malformed_container_probe_unknown",
        ),
        ("new_memory_face", "fterrdef.FT_Err_Unknown_File_Format.face_open_unknown_format"),
        ("sfnt.new_memory_face", "fterrdef.FT_Err_Horiz_Header_Missing.sfnt_missing_hhea_table"),
        (
            "sfnt.new_memory_face",
            "fterrdef.FT_Err_Invalid_Frame_Read.stream_frame_bounds_rejected",
        ),
    }
    if (row.operation, row.case_id) in face_table_error_cases:
        return "face/table opening public error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "smooth.render_error_probe"
        and row.case_id == "ftmoderr.FT_Mod_Err_Smooth.prefixed_error_base"
    ):
        return "Smooth renderer module-prefixed error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id == "freetype.FT_Load_Glyph.matrix_load"
    ):
        return "FT_Load_Glyph matrix-load error rows validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id
        == "freetype.FT_Load_Glyph.error_out_of_range_null_face_or_invalid_flags"
    ):
        return "FT_Load_Glyph out-of-range/null-face/invalid-flag errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id == "freetype.FT_LOAD_FORCE_AUTOHINT.load_glyph_force_autohint_behavior"
    ):
        return "FT_LOAD_FORCE_AUTOHINT load-glyph behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id == "freetype.FT_LOAD_PEDANTIC.pedantic_error_behavior"
    ):
        return "FT_LOAD_PEDANTIC load-glyph errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id == "freetype.FT_LOAD_TARGET_MODE.render_rejects_invalid_target_mode"
    ):
        return "FT_LOAD_TARGET_MODE invalid render-target errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    render_raster_error_cases = {
        (
            "FT_Outline_Render",
            "fterrdef.FT_Err_Cannot_Render_Glyph.outline_raster_unsupported_mode_returns_error",
        ),
        (
            "FT_Outline_Render",
            "fterrdef.FT_Err_Raster_Corrupted.bsdf_empty_contours_corrupted",
        ),
        (
            "FT_Outline_Render",
            "fterrdef.FT_Err_Raster_Corrupted.sdf_raster_missing_flag",
        ),
        (
            "FT_Outline_Render",
            "fterrdef.FT_Err_Raster_Negative_Height.monochrome_raster_negative_height",
        ),
        (
            "FT_Outline_Render",
            "fterrdef.FT_Err_Raster_Uninitialized.raster_render_without_pool",
        ),
        (
            "renderer.raster_render",
            "ftimage.FT_RASTER_FLAG_SDF.non_sdf_raster_rejects_sdf_shape",
        ),
        ("renderer.raster_render", "ftimage.FT_Raster.null_raster_errors"),
        (
            "renderer.raster_render",
            "ftimage.FT_Raster_Funcs.render_callback_error_contract",
        ),
        ("renderer.raster_render", "ftimage.FT_Raster_Params.invalid_param_errors"),
        ("ftoutln.outline_get_bitmap", "ftimage.FT_Bitmap.invalid_target_buffer_errors"),
        (
            "ftoutln.outline_get_bitmap",
            "ftimage.FT_PIXEL_MODE_NONE.invalid_render_target_errors",
        ),
        (
            "ftoutln.outline_get_bitmap",
            "ftoutln.FT_Outline_Get_Bitmap.null_bitmap_and_delegate_errors",
        ),
        ("ftoutln.outline_render", "ftimage.FT_RASTER_FLAG_DIRECT.mono_rejects_direct"),
        (
            "truetype.load_glyph_with_bytecode",
            "fterrdef.FT_Err_Invalid_CodeRange.truetype_invalid_coderange",
        ),
        (
            "FT_New_Memory_Face_then_FT_Load_Glyph",
            "fterrdef.FT_Err_Locations_Missing.glyf_present_loca_missing",
        ),
        (
            "FTC_Manager_RegisterCache",
            "fterrdef.FT_Err_Too_Many_Caches.cache_manager_cache_limit",
        ),
        (
            "cff_or_type1.load_glyph",
            "fterrdef.FT_Err_Ignore.parser_ignore_sentinel_not_public_success",
        ),
        (
            "FT_Load_Glyph_or_FT_Render_Glyph",
            "fterrdef.FT_Err_Invalid_Stream_Read.png_embedded_bitmap_read_failure",
        ),
        (
            "FT_Load_Glyph_or_FT_Render_Glyph",
            "fterrdef.FT_Err_Invalid_SVG_Document.svg_document_failure_policy",
        ),
    }
    if (row.operation, row.case_id) in render_raster_error_cases:
        return "render/raster public error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftbbox.outline_get_bbox"
        and row.case_id == "ftbbox.FT_Outline_Get_BBox.error_null_outline_or_output"
    ):
        return "FT_Outline_Get_BBox null-outline/output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.get_glyph_name"
        and row.case_id == "fterrdef.FT_Err_Invalid_Argument.null_output_or_bad_flag_arguments"
    ):
        return "FT_Get_Glyph_Name null-output error validates public FT_Error in output.status through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.glyph_get_cbox"
        and row.case_id == "ftglyph.FT_Glyph_Get_CBox.success_null_or_no_bbox_zeroes_output"
    ):
        return "FT_Glyph_Get_CBox zeroes non-null acbox before null glyph/class/bbox-hook exits through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.get_glyph"
        and row.case_id == "ftglyph.FT_Get_Glyph.error_null_slot_or_output"
    ):
        return "FT_Get_Glyph null slot/output errors validate exact public FT_Error rows through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.case_id == "ftglyph.FT_Get_Glyph.error_advance_out_of_16_16_range":
        return "FT_Get_Glyph advance overflow and just-inside 26.6 boundaries validate exact public FT_Error, glyph output pointer class, and adjacent successful 16.16 advance conversion through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.glyph_copy"
        and row.case_id == "ftglyph.FT_Glyph_Copy.error_null_source_target_or_class"
    ):
        return "FT_Glyph_Copy null source/target/class errors validate exact public FT_Error rows and target preservation through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.glyph_to_bitmap"
        and row.case_id
        == "ftglyph.FT_Glyph_To_Bitmap.error_invalid_arguments_or_unrenderable_format"
        and row.params.get("null_the_glyph") is True
        and row.params.get("null_deref_glyph") is True
        and row.params.get("null_library") is True
        and row.params.get("null_clazz") is True
        and row.params.get("no_prepare_hook") is True
    ):
        return "FT_Glyph_To_Bitmap early invalid argument paths validate exact public FT_Error rows and caller-handle preservation through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.glyph_to_bitmap"
        and row.case_id == "ftglyph.FT_Glyph_To_Bitmap.error_render_failure_preserves_original"
    ):
        return (
            "FT_Glyph_To_Bitmap render-failure preservation validates exact "
            "public FT_Error, partial-bitmap cleanup, and original glyph "
            "preservation through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "new_memory_face"
        and row.case_id == "fterrdef.FT_Err_Invalid_Library_Handle.library_api_rejects_null_library"
    ):
        return "FT_New_Memory_Face null-library error validates public FT_Error in output.status through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "render_glyph"
        and row.case_id
        == "fterrdef.FT_Err_Invalid_Glyph_Format.render_or_load_rejects_unsupported_glyph_format"
    ):
        return "FT_Render_Glyph unsupported-format error validates public FT_Error in output.status through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "render_glyph"
        and row.case_id == "fterrdef.FT_Err_Missing_SVG_Hooks.svg_render_without_hooks"
    ):
        return "FT_Render_Glyph missing-SVG-hooks error validates public FT_Error in output.status through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftbdf.get_bdf_charset_id"
        and row.case_id == "ftbdf.FT_Get_BDF_Charset_ID.error_non_bdf_face"
    ):
        return "FT_Get_BDF_Charset_ID non-BDF-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftbdf.get_bdf_charset_id"
        and row.case_id == "ftbdf.FT_Get_BDF_Charset_ID.success_bdf_face_charset"
    ):
        return "FT_Get_BDF_Charset_ID BDF charset strings validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftbdf.get_bdf_charset_id"
        and row.case_id
        == "ftbdf.FT_Get_BDF_Charset_ID.error_sfnt_bdf_without_selected_strike"
    ):
        return "FT_Get_BDF_Charset_ID unselected-strike errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftbdf.get_bdf_charset_id"
        and row.case_id == "ftbdf.FT_Get_BDF_Charset_ID.error_null_face_or_outputs"
    ):
        return "FT_Get_BDF_Charset_ID null-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_color_glyph_paint"
        and row.case_id == "ftcolor.FT_COLOR_ROOT_TRANSFORM_MAX.invalid_runtime_behavior"
    ):
        return "FT_COLOR_ROOT_TRANSFORM_MAX rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_paint_malformed"
        and row.case_id
        == "ftcolor.FT_COLR_PAINTFORMAT_UNSUPPORTED.invalid_format_returns_false"
    ):
        return "FT_COLR_PAINTFORMAT_UNSUPPORTED rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_paint_malformed"
        and row.case_id
        == "ftcolor.FT_COLR_PAINT_FORMAT_MAX.read_paint_rejects_max_and_above"
    ):
        return "FT_COLR_PAINT_FORMAT_MAX rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_color_glyph_clipbox"
        and row.case_id == "ftcolor.FT_Get_Color_Glyph_ClipBox.null_and_non_sfnt_rejected"
    ):
        return "FT_Get_Color_Glyph_ClipBox null/non-SFNT rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_color_glyph_clipbox"
        and row.case_id
        == "ftcolor.FT_Get_Color_Glyph_ClipBox.malformed_clipbox_false_behavior"
    ):
        return "FT_Get_Color_Glyph_ClipBox malformed clipbox rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_color_glyph_layer"
        and row.case_id == "ftcolor.FT_Get_Color_Glyph_Layer.invalid_inputs_rejected"
    ):
        return "FT_Get_Color_Glyph_Layer invalid-input rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_color_glyph_layer"
        and row.case_id
        == "ftcolor.FT_Get_Color_Glyph_Layer.malformed_layer_record_false_behavior"
    ):
        return "FT_Get_Color_Glyph_Layer malformed-layer rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation == "ftcolor.get_color_glyph_layer" and row.case_id in {
        "ftcolor.FT_Get_Color_Glyph_Layer.layer_iteration_success",
        "ftcolor.FT_Get_Color_Glyph_Layer.foreground_color_index",
        "ftcolor.FT_Get_Color_Glyph_Layer.terminal_false_preserves_last_outputs",
        "ftcolor.FT_LayerIterator.initialized_and_advanced_by_color_glyph_layers_v0",
    }:
        return "FT_Get_Color_Glyph_Layer COLR v0 layer iterator output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_color_glyph_paint"
        and row.case_id
        == "ftcolor.FT_Get_Color_Glyph_Paint.missing_or_invalid_root_returns_false"
    ):
        return "FT_Get_Color_Glyph_Paint missing/invalid-root rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_color_glyph_paint"
        and row.case_id
        == "ftcolor.FT_Get_Color_Glyph_Paint.non_null_opaque_paint_rejected"
    ):
        return "FT_Get_Color_Glyph_Paint non-null opaque paint rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_color_glyph_paint"
        and row.case_id
        == "ftcolor.FT_Get_Color_Glyph_Paint.null_and_non_sfnt_rejected"
    ):
        return "FT_Get_Color_Glyph_Paint null/non-SFNT rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_colorline_stops"
        and row.case_id
        == "ftcolor.FT_Get_Colorline_Stops.error_null_or_invalid_iterator"
    ):
        return "FT_Get_Colorline_Stops null/invalid-iterator rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_colorline_stops"
        and row.case_id
        == "ftcolor.FT_Get_Colorline_Stops.error_null_color_stop_policy"
    ):
        return "FT_Get_Colorline_Stops null color-stop policy validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_paint"
        and row.case_id == "ftcolor.FT_Get_Paint.error_null_or_missing_colr"
    ):
        return "FT_Get_Paint null/missing-COLR rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_paint"
        and row.case_id == "ftcolor.FT_Get_Paint.error_null_output_policy"
    ):
        return "FT_Get_Paint null-output policy validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_paint_layers"
        and row.case_id
        == "ftcolor.FT_Get_Paint_Layers.error_invalid_iterator_or_paint_offset"
    ):
        return "FT_Get_Paint_Layers invalid-iterator/paint-offset rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_paint_layers"
        and row.case_id == "ftcolor.FT_Get_Paint_Layers.error_null_arguments_policy"
    ):
        return "FT_Get_Paint_Layers null-argument policy validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.get_paint_layers"
        and row.case_id
        == "ftcolor.FT_LayerIterator.initialized_and_advanced_by_layer_apis"
    ):
        return "FT_LayerIterator COLR v0 FT_Get_Color_Glyph_Layer and COLR v1 FT_Get_Paint_Layers iterator advancement validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_data_get"
        and row.case_id == "ftcolor.FT_Palette_Data_Get.error_null_face_or_output"
    ):
        return "FT_Palette_Data_Get null-face/output rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_data_get"
        and row.case_id == "ftcolor.FT_Palette_Data_Get.success_sfnt_cpal_palette_data"
    ):
        return "FT_Palette_Data_Get CPAL palette counts and pointer nullness validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_data_get"
        and row.case_id == "ftcolor.FT_Palette_Data_Get.success_non_sfnt_null_palette_data"
    ):
        return "FT_Palette_Data_Get non-SFNT null palette data validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.case_id in {
        "ftcolor.FT_PALETTE_FOR_DARK_BACKGROUND.palette_flags_runtime",
        "ftcolor.FT_PALETTE_FOR_LIGHT_BACKGROUND.palette_flags_runtime",
        "ftcolor.FT_Palette_Data.palette_data_get_values",
    }:
        return "FT_Palette_Data CPAL name-id, flag, and entry-label arrays validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_select"
        and row.case_id
        == "ftcolor.FT_Palette_Select.error_null_face_or_invalid_palette_index"
    ):
        return "FT_Palette_Select null-face/invalid-index rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_select"
        and row.case_id == "ftcolor.FT_Palette_Select.success_non_sfnt_returns_null_palette"
    ):
        return "FT_Palette_Select non-SFNT null palette output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.case_id in {
        "ftcolor.FT_Color.palette_entries_preserve_bgra_order",
        "ftcolor.FT_Palette_Select.success_selects_palette_and_returns_entries",
    }:
        return "FT_Palette_Select CPAL palette entry BGRA output validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_select"
        and row.case_id
        == "ftcolor.FT_Palette_Select.success_null_output_selects_without_return"
    ):
        return "FT_Palette_Select null-output CPAL selection side effect validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_select"
        and row.case_id
        == "ftcolor.FT_Palette_Select.success_reselect_resets_user_modifications"
    ):
        return "FT_Palette_Select CPAL reselection reset behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_set_foreground_color"
        and row.case_id == "ftcolor.FT_Palette_Set_Foreground_Color.error_null_face"
    ):
        return "FT_Palette_Set_Foreground_Color null-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_set_foreground_color"
        and row.case_id == "ftcolor.FT_Palette_Set_Foreground_Color.success_non_sfnt_noop"
    ):
        return "FT_Palette_Set_Foreground_Color non-SFNT no-op behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_set_foreground_color"
        and row.case_id
        == "ftcolor.FT_Palette_Set_Foreground_Color.success_sets_sfnt_foreground_color"
    ):
        return "FT_Palette_Set_Foreground_Color SFNT foreground-color state and public COLR foreground 0xFFFF PaintSolid reference validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcolor.palette_set_foreground_color"
        and row.case_id
        == "ftcolor.FT_Palette_Set_Foreground_Color.default_foreground_color_policy"
    ):
        return "FT_Palette_Set_Foreground_Color default COLR foreground policy for dark-background and non-dark CPAL palette flags validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcid.get_cid_from_glyph_index"
        and row.case_id
        == "ftcid.FT_Get_CID_From_Glyph_Index.non_cid_or_null_face_errors_and_clears_output"
    ):
        return "FT_Get_CID_From_Glyph_Index non-CID/null-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcid.get_cid_is_internally_cid_keyed"
        and row.case_id
        == "ftcid.FT_Get_CID_Is_Internally_CID_Keyed.non_cid_or_null_face_errors_and_clears_output"
    ):
        return "FT_Get_CID_Is_Internally_CID_Keyed non-CID/null-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcid.get_cid_registry_ordering_supplement"
        and row.case_id
        == "ftcid.FT_Get_CID_Registry_Ordering_Supplement.error_non_cid_or_null_outputs"
    ):
        return "FT_Get_CID_Registry_Ordering_Supplement non-CID/null-output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.cmap_cache_lookup"
        and row.case_id == "ftcache.FTC_CMapCache_Lookup.error_null_cache_returns_zero"
    ):
        return "FTC_CMapCache_Lookup null-cache zero result validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.cmap_cache_lookup"
        and row.case_id
        in {
            "ftcache.FTC_CMapCache_Lookup.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_CMapCache_Lookup.success_lookup_hit_and_repeat_hit",
            "ftcache.FTC_CMapCache_Lookup.success_lookup_miss_returns_zero",
            "ftcache.FTC_CMapCache_Lookup.success_negative_cmap_index_uses_current_charmap",
            "ftcache.FTC_CMapCache_Lookup.lifecycle_remove_faceid_and_reset",
        }
    ):
        return (
            "FTC_CMapCache_Lookup glyph-index, repeat lookup, requester-count, "
            "negative-cmap-index, remove-face-id, and reset behavior validates "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.cmap_cache_new"
        and row.case_id == "ftcache.FTC_CMapCache_New.error_null_manager_or_output"
    ):
        return "FTC_CMapCache_New null-manager/output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.cmap_cache_new"
        and row.case_id
        in {
            "ftcache.FTC_CMapCache_New.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_CMapCache_New.success_create_and_destroy_with_manager",
            "ftcache.FTC_CMapCache_New.success_multiple_cache_registration_limit",
            "ftcache.FTC_CMapCache_New.lifecycle_after_manager_reset",
        }
    ):
        return (
            "FTC_CMapCache_New create, manager-owned destruction, lookup "
            "usability, registration limit, and reset-preserved cache handle "
            "behavior validate through pinned C oracle, Rust FFI, C ABI, and "
            "WASM ABI"
        )
    if (
        row.operation == "ftcache.image_cache_lookup"
        and row.case_id == "ftcache.FTC_ImageCache_Lookup.error_null_aglyph"
    ):
        return "FTC_ImageCache_Lookup null-aglyph errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.image_cache_lookup"
        and row.case_id
        == "ftcache.FTC_ImageCache_Lookup.error_invalid_cache_type_face_or_glyph"
    ):
        return "FTC_ImageCache_Lookup invalid-cache/type/face/glyph errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.image_cache_lookup"
        and row.case_id
        in {
            "ftcache.FTC_ImageCache_Lookup.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_ImageCache_Lookup.success_lookup_hit_and_repeat_hit",
            "ftcache.FTC_ImageCache_Lookup.success_node_acquire_and_unref",
            "ftcache.FTC_ImageCache_Lookup.success_null_anode_ephemeral_glyph",
        }
    ):
        return (
            "FTC_ImageCache_Lookup image-type sizing, glyph output, repeat "
            "lookup, requester count, null/non-null anode ownership "
            "classification, and node-unref behavior validate through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation in {"ftcache.node_lifecycle", "ftcache.node_unref"}
        and row.case_id
        in {
            "ftcache.FTC_Node.reference_counted_cache_handle",
            "ftcache.FTC_Node_Unref.null_inputs_noop",
            "ftcache.FTC_Node_Unref.null_or_invalid_inputs_noop",
            "ftcache.FTC_Node_Unref.releases_lookup_reference",
            "ftcache.FTC_Node_Unref.unreferenced_node_becomes_flushable",
        }
    ):
        if row.case_id == "ftcache.FTC_Node_Unref.null_inputs_noop":
            return (
                "FTC_Node_Unref null node/null manager no-op validates through "
                "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
            )
        if row.case_id == "ftcache.FTC_Node_Unref.null_or_invalid_inputs_noop":
            return (
                "FTC_Node_Unref null node/live manager and foreign "
                "out-of-range cache-index no-op behavior validates through "
                "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
            )
        return (
            "FTC_Node and FTC_Node_Unref lookup-acquired node handle, cache "
            "index, reference release, and post-unref flushability classes "
            "validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.image_cache_lookup_scaler"
        and row.case_id
        == "ftcache.FTC_ImageCache_LookupScaler.error_null_scaler_or_aglyph"
    ):
        return "FTC_ImageCache_LookupScaler null-scaler/aglyph errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.image_cache_lookup_scaler"
        and row.case_id
        in {
            "ftcache.FTC_ImageCache_LookupScaler.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_ImageCache_LookupScaler.success_pixel_and_point_scalers",
            "ftcache.FTC_ImageCache_LookupScaler.success_lookup_hit_miss_and_repeated",
            "ftcache.FTC_ImageCache_LookupScaler.success_node_acquire_and_unref",
            "ftcache.FTC_ImageCache_LookupScaler.load_flags_truncation_policy",
        }
    ):
        return (
            "FTC_ImageCache_LookupScaler scaler size selection, glyph hit/miss, "
            "effective load-flag truncation, glyph public record, and node-unref "
            "classification validate through pinned C oracle, Rust FFI, C ABI, "
            "and WASM ABI"
        )
    if (
        row.operation == "ftcache.sbit_cache_lookup"
        and row.case_id == "ftcache.FTC_SBitCache_Lookup.rejects_null_sbit_output"
    ):
        return "FTC_SBitCache_Lookup null-sbit-output error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.sbit_cache_lookup"
        and row.case_id == "ftcache.FTC_SBitCache_Lookup.clears_outputs_before_lookup"
    ):
        return "FTC_SBitCache_Lookup output-clearing error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.image_cache_new"
        and row.case_id == "ftcache.FTC_ImageCache_New.error_null_manager_or_output"
    ):
        return "FTC_ImageCache_New null-manager/output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.image_cache_new"
        and row.case_id == "ftcache.FTC_ImageCache_New.error_too_many_caches"
    ):
        return "FTC_ImageCache_New too-many-caches errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.image_cache_new"
        and row.case_id
        in {
            "ftcache.FTC_ImageCache_New.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_ImageCache_New.success_create_lookup_destroy_lifecycle",
            "ftcache.FTC_ImageCache_New.success_manager_reset_preserves_handle",
        }
    ):
        return (
            "FTC_ImageCache_New create, glyph lookup, manager-owned "
            "destruction, node-unref classification, and reset-preserved cache "
            "handle behavior validate through pinned C oracle, Rust FFI, C "
            "ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.manager_lookup_face"
        and row.case_id == "ftcache.FTC_Manager_LookupFace.error_null_output_or_manager"
    ):
        return "FTC_Manager_LookupFace null-output/manager errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.manager_lookup_face"
        and row.case_id == "ftcache.FTC_Manager_LookupFace.error_requester_failure"
    ):
        return "FTC_Manager_LookupFace requester-failure errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.manager_lookup_face"
        and row.case_id
        == "fterrdef.FT_Err_Invalid_Cache_Handle.cache_lookup_rejects_null_manager"
    ):
        return "FTC_Manager_LookupFace null-manager cache-handle errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.manager_lookup_face"
        and row.case_id
        in {
            "ftcache.FTC_Manager_LookupFace.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_Manager_LookupFace.success_first_lookup_invokes_requester",
            "ftcache.FTC_Manager_LookupFace.success_repeat_lookup_returns_cached_face",
            "ftcache.FTC_Manager_LookupFace.success_face_has_no_required_current_size",
        }
    ):
        return (
            "FTC_Manager_LookupFace requester-count, cache identity class, "
            "RemoveFaceID reload behavior, and public face fields validate "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.manager_lookup_size"
        and row.case_id
        == "ftcache.FTC_Manager_LookupSize.error_null_scaler_output_or_manager"
    ):
        return "FTC_Manager_LookupSize null-scaler/output/manager errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.manager_lookup_size"
        and row.case_id
        == "ftcache.FTC_Manager_LookupSize.error_requester_or_size_selection_failure"
    ):
        return "FTC_Manager_LookupSize requester/size-selection errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.manager_lookup_size"
        and row.case_id
        in {
            "ftcache.FTC_Manager_LookupSize.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_Manager_LookupSize.success_pixel_size_scaler",
            "ftcache.FTC_Manager_LookupSize.success_point_size_resolution_scaler",
            "ftcache.FTC_Manager_LookupSize.success_repeat_lookup_cached_size",
            "ftcache.FTC_ScalerRec.pixel_scaler_uses_integer_pixels",
            "ftcache.FTC_ScalerRec.point_scaler_uses_26_6_points_and_resolution",
        }
    ):
        return (
            "FTC_Manager_LookupSize scaler metrics, requester-count, and "
            "immediate repeat identity classification validate through pinned "
            "C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.manager_new"
        and row.case_id == "ftcache.FTC_Manager_New.error_null_library"
    ):
        return "FTC_Manager_New null-library errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.manager_new"
        and row.case_id == "ftcache.FTC_Manager_New.error_null_requester_or_output"
    ):
        return "FTC_Manager_New null-requester/output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.manager_new"
        and row.case_id
        in {
            "ftcache.FTC_Manager_New.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_Manager_New.success_defaults_for_zero_limits",
            "ftcache.FTC_Manager_New.success_custom_limits_and_req_data",
            "ftcache.FTC_Manager_New.lifecycle_create_lookup_reset_done",
        }
    ):
        return (
            "FTC_Manager_New zero/custom limit creation, req_data forwarding, "
            "lookup requester count, reset, and manager-done lifecycle validate "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.manager_remove_face_id"
        and row.case_id
        in {
            "ftcache.FTC_Manager_RemoveFaceID.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_Manager_RemoveFaceID.success_removes_unreferenced_face_size_and_nodes",
            "ftcache.FTC_Manager_RemoveFaceID.success_referenced_nodes_hidden_until_unref",
            "ftcache.FTC_Manager_RemoveFaceID.success_other_face_ids_unchanged",
            "ftcache.FTC_Manager_RemoveFaceID.success_null_manager_noop",
            "ftcache.FTC_Manager_RemoveFaceID.success_null_or_unknown_face_id",
        }
    ):
        return (
            "FTC_Manager_RemoveFaceID face-id eviction, distinct face-id "
            "isolation, referenced-node unref sequencing, unknown/null face-id "
            "no-ops, and null-manager no-op behavior validate through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.manager_done"
        and row.case_id
        in {
            "ftcache.FTC_Manager_Done.planned_cache_subsystem_not_out_of_scope",
            "ftcache.FTC_Manager_Done.success_destroy_empty_manager",
            "ftcache.FTC_Manager_Done.success_destroy_populated_manager",
            "ftcache.FTC_Manager_Done.success_null_or_invalid_library_noop",
            "ftcache.FTC_Manager_Done.node_reference_lifecycle_on_done",
        }
    ):
        return (
            "FTC_Manager_Done null-manager no-op, empty-manager teardown, "
            "populated cache/face/size/node release-before-done lifecycle, and "
            "void-return ownership behavior validate through pinned C oracle, "
            "Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.manager_lifecycle"
        and row.case_id == "ftcache.FTC_Manager.reset_and_done_lifecycle"
    ):
        return (
            "FTC_Manager reset/done lifecycle validates through a maintained "
            "same-input route proving reset preserves manager usability and "
            "done tears down populated manager-owned cache/face/size/node "
            "state through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftcache.sbit_cache_lookup_scaler"
        and row.case_id
        == "ftcache.FTC_SBitCache_LookupScaler.rejects_null_sbit_or_scaler"
    ):
        return "FTC_SBitCache_LookupScaler null-sbit/scaler errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.sbit_cache_lookup_scaler"
        and row.case_id
        == "ftcache.FTC_SBitCache_LookupScaler.clears_outputs_before_lookup"
    ):
        return "FTC_SBitCache_LookupScaler output-clearing behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.scaler_descriptor_lifetime"
        and row.case_id == "ftcache.FTC_Scaler.points_to_call_owned_scaler"
    ):
        return "FTC_Scaler caller-owned descriptor lifetime validates through actual pinned C FTC_SBitCache_LookupScaler, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.image_type_descriptor_lifetime"
        and row.case_id == "ftcache.FTC_ImageType.points_to_call_owned_descriptor"
    ):
        return "FTC_ImageType caller-owned descriptor lifetime validates through actual pinned C FTC_ImageCache_Lookup, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.image_type_lookup_probe"
        and row.case_id == "ftcache.FTC_ImageTypeRec.drives_image_and_sbit_lookup"
    ):
        return "FTC_ImageTypeRec image and sbit lookup fields validate through actual pinned C FTC_ImageCache_Lookup and FTC_SBitCache_Lookup, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.sbit_cache_new"
        and row.case_id == "ftcache.FTC_SBitCache_New.error_outputs_null_cache"
    ):
        return "FTC_SBitCache_New null-cache output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.sbit_cache_new"
        and row.case_id == "ftcache.FTC_SBitCache_New.invalid_arguments_match_c"
    ):
        return "FTC_SBitCache_New invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.sbit_cache_new"
        and row.case_id == "ftcache.FTC_SBitCache_New.creates_manager_owned_cache"
    ):
        return "FTC_SBitCache_New successful manager-owned SBit cache handle creation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.type_contract"
        and row.case_id
        in {
            "ftcache.FTC_CMapCache.manager_owned_opaque_cache",
            "ftcache.FTC_ImageCache.manager_owned_opaque_cache",
            "ftcache.FTC_SBitCache.manager_owned_sbit_cache",
        }
    ):
        return "FTC cache opaque public handle nullness, manager ownership identity, and manager-done lifecycle class validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftcache.face_id_identity"
        and row.case_id == "ftcache.FTC_FaceID.pointer_identity_key"
    ):
        return "FTC_FaceID raw pointer identity, same-bytes distinct-pointer miss, and same-address alias hit behavior validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftbzip2.stream_open_bzip2"
        and row.case_id
        == "ftbzip2.FT_Stream_OpenBzip2.out_of_scope_uncompiled_bzip2_policy"
    ):
        return "FT_Stream_OpenBzip2 disabled-bzip2 build policy validates Unimplemented_Feature through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftbzip2.stream_open_bzip2"
        and row.case_id
        in {
            "ftbzip2.FT_Stream_OpenBzip2.disabled_build_precedes_null_validation",
            "ftbzip2.FT_Stream_OpenBzip2.disabled_build_precedes_header_validation",
        }
    ):
        return "FT_Stream_OpenBzip2 active disabled-bzip2 build precedence validates Unimplemented_Feature before enabled-build input validation through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgxval.classic_kern_validate"
        and row.case_id == "ftgxval.FT_ClassicKern_Validate.rejects_invalid_arguments"
    ):
        return "FT_ClassicKern_Validate invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgxval.classic_kern_validate"
        and row.case_id
        == "ftgxval.FT_ClassicKern_Validate.reports_unimplemented_or_invalid_table"
    ):
        return "FT_ClassicKern_Validate unimplemented/invalid-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgxval.classic_kern_validate"
        and row.case_id == "ftgxval.FT_VALIDATE_APPLE.absent_or_invalid_kern_table"
    ):
        return "FT_VALIDATE_APPLE absent/invalid-kern-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgxval.classic_kern_validate"
        and row.case_id == "ftgxval.FT_VALIDATE_CKERN.malformed_table_error_matches_c"
    ):
        return "FT_VALIDATE_CKERN malformed-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.new_glyph"
        and row.case_id == "ftglyph.FT_New_Glyph.error_null_library_or_output"
    ):
        return "FT_New_Glyph null-library/output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.new_glyph"
        and row.case_id == "ftglyph.FT_New_Glyph.error_unsupported_format"
    ):
        return "FT_New_Glyph unsupported-format errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.new_glyph"
        and row.case_id == "ftglyph.FT_New_Glyph.error_allocation_failure"
    ):
        return "FT_New_Glyph allocation-failure errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.glyph_transform"
        and row.case_id == "ftglyph.FT_Glyph_Transform.error_null_or_bad_glyph"
    ):
        return "FT_Glyph_Transform null/bad-glyph errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.glyph_transform"
        and row.case_id == "ftglyph.FT_Glyph_Transform.error_non_scalable_bitmap"
    ):
        return "FT_Glyph_Transform non-scalable bitmap errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.glyph_transform"
        and row.case_id
        in {
            "ftglyph.FT_Glyph_Transform.success_outline_matrix_delta",
            "ftglyph.FT_Glyph_Transform.success_outline_delta_only_or_matrix_only",
        }
    ):
        return "FT_Glyph_Transform outline success validates owned FT_Get_Glyph outline records, transform status, outline points, root advance, CBox, and mutation class through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlist.list_iterate"
        and row.case_id == "ftlist.FT_List_Iterate.iterates_all_nodes_success"
    ):
        return "FT_List_Iterate success traversal validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlist.list_iterate"
        and row.case_id == "ftlist.FT_List_Iterate.iterator_can_mutate_current_node"
    ):
        return "FT_List_Iterate callback mutation validates snapshot-next traversal through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlist.list_iterate"
        and row.case_id == "ftlist.FT_List_Iterate.stops_on_callback_error"
    ):
        return "FT_List_Iterate callback-error propagation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlist.list_iterate"
        and row.case_id == "ftlist.FT_List_Iterate.null_list_or_iterator_error"
    ):
        return "FT_List_Iterate null-list/iterator errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftrender.set_renderer"
        and row.case_id == "ftrender.FT_Set_Renderer.set_outline_renderer_success"
    ):
        return (
            "FT_Set_Renderer default outline renderer success validates "
            "FT_Get_Renderer-selected library-owned renderer handle, OK status, "
            "and current renderer class through pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI; custom renderer set_mode and rendered-output "
            "mutation remain pending"
        )
    if (
        row.operation == "ftrender.set_renderer"
        and row.case_id == "ftrender.FT_Set_Renderer.invalid_library_renderer_or_params"
    ):
        return "FT_Set_Renderer invalid-library/renderer/params errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftrender.set_renderer"
        and row.case_id == "ftrender.FT_Set_Renderer.set_mode_parameter_error_propagates"
    ):
        return "FT_Set_Renderer set-mode parameter-error propagation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftsnames.get_sfnt_lang_tag"
        and row.case_id == "ftsnames.FT_Get_Sfnt_LangTag.invalid_argument_errors"
    ):
        return "FT_Get_Sfnt_LangTag invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.stroker_new"
        and row.case_id == "ftstroke.FT_Stroker_New.invalid_library"
    ):
        return "FT_Stroker_New invalid-library errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.stroker_new"
        and row.case_id == "ftstroke.FT_Stroker_New.invalid_output_pointer"
    ):
        return "FT_Stroker_New invalid-output-pointer errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.stroker_new"
        and row.case_id == "ftstroke.FT_Stroker_New.allocation_failure"
    ):
        return "FT_Stroker_New allocation-failure errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.parse_outline"
        and row.case_id == "ftstroke.FT_Stroker_ParseOutline.invalid_outline"
    ):
        return "FT_Stroker_ParseOutline invalid-outline errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.parse_outline"
        and row.case_id == "ftstroke.FT_Stroker_ParseOutline.invalid_stroker"
    ):
        return "FT_Stroker_ParseOutline invalid-stroker errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftwinfnt.get_winfnt_header"
        and row.case_id
        == "ftwinfnt.FT_Get_WinFNT_Header.null_face_returns_invalid_face_handle"
    ):
        return "FT_Get_WinFNT_Header null-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftwinfnt.get_winfnt_header"
        and row.case_id
        == "ftwinfnt.FT_Get_WinFNT_Header.null_output_returns_invalid_argument"
    ):
        return "FT_Get_WinFNT_Header null-output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftwinfnt.get_winfnt_header"
        and row.case_id
        == "ftwinfnt.FT_Get_WinFNT_Header.non_winfnt_face_returns_invalid_argument"
    ):
        return "FT_Get_WinFNT_Header non-WinFNT-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftwinfnt.get_winfnt_header"
        and row.case_id
        in {
            "ftwinfnt.FT_Get_WinFNT_Header.winfnt_face_copies_header_success",
            "ftwinfnt.FT_WinFNT_HeaderRec.copied_header_values_match_file",
        }
        and not any(
            unresolved_asset_reason(asset, name) for name, asset in sorted(row.assets.items())
        )
    ):
        return "FT_Get_WinFNT_Header success header copy validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftwinfnt.get_winfnt_header_mutation"
        and row.case_id == "ftwinfnt.FT_WinFNT_Header.mutable_output_handle_contract"
    ):
        return "FT_Get_WinFNT_Header caller-owned output mutation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "winfnt.charmap_probe"
        and row.case_id == "ftwinfnt.FT_WinFNT_ID_MAC.mac_charset_selects_apple_roman_charmap"
    ):
        return "WinFNT charset 77 charmap selection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftoutln.outline_check"
        and row.case_id == "ftoutln.FT_Outline_Check.invalid_null_or_count_mismatch"
    ):
        return "FT_Outline_Check null/count-mismatch errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgxval.truetype_gx_validate"
        and row.case_id == "ftgxval.FT_TrueTypeGX_Validate.rejects_invalid_arguments"
    ):
        return "FT_TrueTypeGX_Validate invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgxval.truetype_gx_validate"
        and row.case_id
        == "ftgxval.FT_TrueTypeGX_Validate.reports_unimplemented_or_invalid_table"
    ):
        return "FT_TrueTypeGX_Validate unimplemented/invalid-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.gzip_uncompress"
        and row.case_id == "ftgzip.FT_Gzip_Uncompress.uncompresses_valid_gzip_buffer"
    ):
        return "FT_Gzip_Uncompress gzip/zlib success validates exact bytes through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.gzip_uncompress"
        and row.case_id == "ftgzip.FT_Gzip_Uncompress.rejects_invalid_arguments"
    ):
        return "FT_Gzip_Uncompress invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.gzip_uncompress"
        and row.case_id == "ftgzip.FT_Gzip_Uncompress.reports_buffer_too_small"
    ):
        return "FT_Gzip_Uncompress buffer-too-small errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.gzip_uncompress"
        and row.case_id == "ftgzip.FT_Gzip_Uncompress.reports_invalid_compressed_data"
    ):
        return "FT_Gzip_Uncompress invalid-compressed-data errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.gzip_uncompress"
        and row.case_id == "ftgzip.FT_Gzip_Uncompress.reports_unimplemented_without_zlib"
    ):
        return "FT_Gzip_Uncompress no-zlib unimplemented errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.stream_open_gzip"
        and row.case_id == "ftgzip.FT_Stream_OpenGzip.opens_valid_gzip_stream"
    ):
        return "FT_Stream_OpenGzip small/large gzip stream success validates stream classes and exact range reads through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.stream_open_gzip"
        and row.case_id == "ftgzip.FT_Stream_OpenGzip.rejects_invalid_stream_handles"
    ):
        return "FT_Stream_OpenGzip invalid-stream-handle errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.stream_open_gzip"
        and row.case_id == "ftgzip.FT_Stream_OpenGzip.rejects_invalid_gzip_header"
    ):
        return "FT_Stream_OpenGzip invalid-header errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftgzip.stream_open_gzip"
        and row.case_id == "ftgzip.FT_Stream_OpenGzip.reports_unimplemented_without_zlib"
    ):
        return "FT_Stream_OpenGzip no-zlib unimplemented errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlzw.stream_open_lzw"
        and row.case_id == "ftlzw.FT_Stream_OpenLZW.invalid_header_error"
    ):
        return "FT_Stream_OpenLZW invalid-header errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlzw.stream_open_lzw"
        and row.case_id == "ftlzw.FT_Stream_OpenLZW.null_stream_or_source_error"
    ):
        return "FT_Stream_OpenLZW null-stream/source errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlzw.stream_open_lzw"
        and row.case_id == "ftlzw.FT_Stream_OpenLZW.unsupported_build_error"
    ):
        return "FT_Stream_OpenLZW unsupported-build errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_OpenType_Validate.service_missing_error"
    ):
        return "FT_OpenType_Validate missing-service errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_OpenType_Validate.malformed_table_error"
    ):
        return "FT_OpenType_Validate malformed-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_OpenType_Validate.selected_tables_success"
    ):
        return "FT_OpenType_Validate selected-table success validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_VALIDATE_GDEF.malformed_table_error"
    ):
        return "FT_VALIDATE_GDEF malformed-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_VALIDATE_GPOS.malformed_table_error"
    ):
        return "FT_VALIDATE_GPOS malformed-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_VALIDATE_GSUB.malformed_table_error"
    ):
        return "FT_VALIDATE_GSUB malformed-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_VALIDATE_JSTF.absent_or_malformed_table"
    ):
        return "FT_VALIDATE_JSTF absent/malformed-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_VALIDATE_MATH.absent_or_malformed_table"
    ):
        return "FT_VALIDATE_MATH absent/malformed-table errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftotval.open_type_validate"
        and row.case_id == "ftotval.FT_VALIDATE_OT.partial_failure_cleanup_contract"
    ):
        return "FT_VALIDATE_OT partial-failure cleanup validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.begin_subpath"
        and row.case_id == "ftstroke.FT_Stroker_BeginSubPath.invalid_arguments"
    ):
        return "FT_Stroker_BeginSubPath invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.begin_subpath"
        and row.case_id
        in {
            "ftstroke.FT_Stroker_BeginSubPath.closed_subpath_initial_state",
            "ftstroke.FT_Stroker_BeginSubPath.open_subpath_initial_state",
        }
    ):
        return "FT_Stroker_BeginSubPath initial-state routes validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.begin_subpath_wide_stroke"
        and row.case_id
        == "ftstroke.FT_Stroker_BeginSubPath.wide_stroke_mode_depends_on_cap_and_join"
    ):
        return "FT_Stroker_BeginSubPath wide-stroke setup and open-path finalization validate exported outlines through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.conic_to"
        and row.case_id == "ftstroke.FT_Stroker_ConicTo.conic_curve_success"
    ):
        return (
            "FT_Stroker_ConicTo maintained quadratic curve route validates exact "
            "status, exported outline points, tags, contours, and CBox through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.conic_to"
        and row.case_id == "ftstroke.FT_Stroker_ConicTo.first_segment_starts_subpath"
    ):
        return (
            "FT_Stroker_ConicTo open first-segment route validates exact status "
            "sequence, exported outline points, tags, contours, and CBox through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.conic_to"
        and row.case_id == "ftstroke.FT_Stroker_ConicTo.invalid_arguments"
    ):
        return "FT_Stroker_ConicTo invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.cubic_to"
        and row.case_id == "ftstroke.FT_Stroker_CubicTo.cubic_curve_success"
    ):
        return (
            "FT_Stroker_CubicTo maintained cubic curve route validates exact "
            "status sequence, exported outline points, tags, contours, and "
            "CBox through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.cubic_to"
        and row.case_id == "ftstroke.FT_Stroker_CubicTo.first_segment_starts_subpath"
    ):
        return (
            "FT_Stroker_CubicTo open first-segment route validates exact "
            "status sequence, exported outline points, tags, contours, and "
            "CBox through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.cubic_to"
        and row.case_id == "ftstroke.FT_Stroker_CubicTo.invalid_arguments"
    ):
        return "FT_Stroker_CubicTo invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.end_subpath"
        and row.case_id == "ftstroke.FT_Stroker_EndSubPath.invalid_stroker"
    ):
        return "FT_Stroker_EndSubPath invalid-stroker errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.get_border_counts"
        and row.case_id
        == "ftstroke.FT_Stroker_GetBorderCounts.invalid_stroker_or_border"
    ):
        return "FT_Stroker_GetBorderCounts invalid-stroker/border errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.get_counts"
        and row.case_id == "ftstroke.FT_Stroker_GetCounts.invalid_stroker"
    ):
        return "FT_Stroker_GetCounts invalid-stroker errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.glyph_stroke"
        and row.case_id == "ftstroke.FT_Glyph_Stroke.invalid_glyph_arguments"
    ):
        return "FT_Glyph_Stroke invalid-glyph errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.glyph_stroke"
        and row.case_id == "ftstroke.FT_Glyph_Stroke.outline_glyph_stroked_success"
    ):
        return (
            "FT_Glyph_Stroke outline glyph route validates exact replacement "
            "outline points, tags, contours, CBox, status sequence, and "
            "preserve-original ownership through pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.glyph_stroke"
        and row.case_id == "ftstroke.FT_Glyph_Stroke.destroy_original_option"
    ):
        return (
            "FT_Glyph_Stroke destroy-option route validates exact replacement "
            "outline points, tags, contours, CBox, status sequence, and "
            "destroy-original ownership through pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.glyph_stroke"
        and row.case_id
        == "ftstroke.FT_Glyph_Stroke.failure_sets_output_null_when_preserving_original"
    ):
        return "FT_Glyph_Stroke failure output-null policy validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.glyph_stroke_border"
        and row.case_id == "ftstroke.FT_Glyph_StrokeBorder.outside_border_success"
    ):
        return (
            "FT_Glyph_StrokeBorder outside-border route validates exact "
            "selected border, replacement outline points, tags, contours, "
            "CBox, status sequence, and preserve-original ownership through "
            "pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.glyph_stroke_border"
        and row.case_id == "ftstroke.FT_Glyph_StrokeBorder.inside_border_success"
    ):
        return (
            "FT_Glyph_StrokeBorder inside-border route validates exact "
            "orientation-selected border, replacement outline points, tags, "
            "contours, and owner flags through pinned C oracle, Rust FFI, "
            "C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.glyph_stroke_border"
        and row.case_id == "ftstroke.FT_Glyph_StrokeBorder.destroy_original_option"
    ):
        return (
            "FT_Glyph_StrokeBorder destroy-option route validates replacement "
            "glyph nullness and original-glyph destruction through pinned C "
            "oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftstroke.glyph_stroke_border"
        and row.case_id == "ftstroke.FT_Glyph_StrokeBorder.invalid_glyph_arguments"
    ):
        return "FT_Glyph_StrokeBorder invalid-glyph errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftstroke.line_to"
        and row.case_id == "ftstroke.FT_Stroker_LineTo.invalid_arguments"
    ):
        return "FT_Stroker_LineTo invalid-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if row.operation in {
        "ftstroke.open_path_geometry",
        "ftstroke.end_subpath",
        "ftstroke.export_border",
        "ftstroke.export",
        "ftstroke.join_geometry_alias",
    } and row.case_id in {
        "ftstroke.FT_STROKER_LINECAP_BUTT.butt_cap_open_line_geometry",
        "ftstroke.FT_STROKER_LINECAP_ROUND.round_cap_open_line_geometry",
        "ftstroke.FT_STROKER_LINECAP_SQUARE.square_cap_open_line_geometry",
        "ftstroke.FT_Stroker_LineCap.open_path_cap_geometry",
        "ftstroke.FT_STROKER_BORDER_LEFT.left_border_export_geometry",
        "ftstroke.FT_STROKER_BORDER_RIGHT.right_border_export_geometry",
        "ftstroke.FT_StrokerBorder.border_selection_runtime_shape",
        "ftstroke.FT_Stroker_Export.exports_left_then_right",
        "ftstroke.FT_Stroker_ExportBorder.valid_left_and_right_export",
        "ftstroke.FT_Stroker_ExportBorder.open_path_right_border_empty",
        "ftstroke.FT_Stroker_EndSubPath.open_subpath_emits_caps_and_single_border",
        "ftstroke.FT_Stroker_EndSubPath.closed_subpath_closes_two_borders",
    }:
        if row.case_id == "ftstroke.FT_Stroker_EndSubPath.closed_subpath_closes_two_borders":
            return (
                "FT_Stroker_EndSubPath closed two-line path finalization validates "
                "exact left and right exported border outlines through pinned C "
                "oracle, Rust FFI, C ABI, and WASM ABI"
            )
        if row.case_id == "ftstroke.FT_Stroker_EndSubPath.open_subpath_emits_caps_and_single_border":
            return (
                "FT_Stroker_EndSubPath open-path cap emission and single-border "
                "finalization validate exact exported outline points, tags, and "
                "contours through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
            )
        return (
            "FT_Stroker open-line geometry validates exact exported outline "
            "points, tags, contours, empty right border, and combined export "
            "through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
        )
    if (
        row.operation == "ftpfr.get_pfr_advance"
        and row.case_id == "ftpfr.FT_Get_PFR_Advance.non_pfr_returns_invalid_argument"
    ):
        return "FT_Get_PFR_Advance non-PFR-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftpfr.get_pfr_advance"
        and row.case_id == "ftpfr.FT_Get_PFR_Advance.null_face_or_output_errors"
    ):
        return "FT_Get_PFR_Advance null-face/output errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftpfr.get_pfr_kerning"
        and row.case_id == "ftpfr.FT_Get_PFR_Kerning.null_face_or_vector_errors"
    ):
        return "FT_Get_PFR_Kerning null-face/vector errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftpfr.get_pfr_metrics"
        and row.case_id
        == "ftpfr.FT_Get_PFR_Metrics.non_pfr_outputs_valid_values_and_unknown_format"
    ):
        return "FT_Get_PFR_Metrics non-PFR-face output/error behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftpfr.get_pfr_metrics"
        and row.case_id == "ftpfr.FT_Get_PFR_Metrics.optional_outputs_and_null_face"
    ):
        return "FT_Get_PFR_Metrics optional-output/null-face behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftoutln.outline_render"
        and row.case_id == "ftimage.FT_RASTER_FLAG_AA.mono_rejects_aa"
    ):
        return "FT_RASTER_FLAG_AA mono-render rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "render_glyph"
        and row.case_id == "freetype.FT_Render_Glyph.invalid_render_mode"
    ):
        return "FT_Render_Glyph invalid render-mode errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "render_glyph"
        and row.case_id
        == "freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format"
    ):
        return "FT_Render_Glyph composite/unsupported slot-format errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "render_glyph"
        and row.case_id
        == "freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format.unrouted_slot_states"
    ):
        return "FT_Render_Glyph unloaded and unsupported synthetic slot-state errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "render_glyph"
        and row.case_id == "freetype.FT_RENDER_MODE_MAX.render_glyph_rejects_sentinel"
    ):
        return "FT_RENDER_MODE_MAX render rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "render_glyph"
        and row.case_id == "fterrdef.FT_Err_Raster_Overflow.raster_buffer_or_cell_overflow"
    ):
        return "FT_Err_Raster_Overflow render-glyph errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftglyph.get_glyph"
        and row.case_id == "ftglyph.FT_Get_Glyph.error_unsupported_synthetic_format"
    ):
        return "FT_Get_Glyph unsupported synthetic slot-format error and output-pointer preservation validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "sfnt.load_sfnt_table"
        and row.case_id == "tttables.FT_Load_Sfnt_Table.missing_table_or_invalid_face_error"
    ):
        return "FT_Load_Sfnt_Table missing-table/invalid-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "sfnt.table_info"
        and row.case_id == "tttables.FT_Sfnt_Table_Info.invalid_index_or_arguments"
    ):
        return "FT_Sfnt_Table_Info invalid-index/argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.face_handle_error_policy"
        and row.case_id == "freetype.FT_Face.null_and_done_handle_errors"
    ):
        return "FT_Face null/done handle error policy validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.face_macro.is_named_instance"
        and row.case_id
        == "freetype.FT_IS_NAMED_INSTANCE.encoded_named_instance_face_index_returns_true"
    ):
        return "FT_IS_NAMED_INSTANCE encoded face-index error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftadvanc.get_advance"
        and row.case_id == "ftadvanc.FT_Get_Advance.error_null_face_or_output"
    ):
        return "FT_Get_Advance null-face/null-output error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftadvanc.get_advance"
        and row.case_id
        == "fterrdef.FT_Err_Invalid_Size_Handle.null_or_detached_size_rejected"
    ):
        return "FT_Get_Advance probe-face invalid-size-handle error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftadvanc.get_advance"
        and row.case_id == "ftadvanc.FT_ADVANCE_FLAG_FAST_ONLY.fast_only_error_behavior"
    ):
        return "FT_ADVANCE_FLAG_FAST_ONLY error behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftadvanc.get_advance"
        and row.case_id == "ftadvanc.FT_Get_Advance.error_invalid_glyph_or_flags"
    ):
        return "FT_Get_Advance invalid-glyph/invalid-flags errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftadvanc.get_advances"
        and row.case_id == "ftadvanc.FT_Get_Advances.error_null_face_or_output"
    ):
        return "FT_Get_Advances null-face/null-output error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftadvanc.get_advances"
        and row.case_id == "ftadvanc.FT_Get_Advances.error_invalid_range_or_flags"
    ):
        return "FT_Get_Advances invalid-range/invalid-flags errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_char_size"
        and row.case_id == "freetype.FT_Set_Char_Size.error_oversized_dimensions"
    ):
        return "FT_Set_Char_Size oversized-dimensions error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_char_size"
        and row.case_id
        == "fterrdef.FT_Err_Invalid_Pixel_Size.set_char_size_rejects_oversized_dimensions"
    ):
        return "FT_Set_Char_Size invalid-pixel-size fterrdef route validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.done_freetype"
        and row.case_id == "freetype.FT_Done_FreeType.error_null_library"
        and lifecycle_handle(row, "library") == "null"
    ):
        return "FT_Done_FreeType null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.done_library"
        and row.case_id == "ftmodapi.FT_Done_Library.rejects_null_library"
        and lifecycle_handle(row, "library") == "null"
    ):
        return "FT_Done_Library null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.reference_library"
        and row.case_id == "ftmodapi.FT_Reference_Library.rejects_null_library"
        and lifecycle_handle(row, "library") == "null"
    ):
        return "FT_Reference_Library null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.add_module"
        and row.case_id == "ftmodapi.FT_Add_Module.add_minimal_module_success"
    ):
        return "FT_Add_Module minimal synthetic module success validates module table insertion, FT_Get_Module lookup, stored class fields, and module_init callback through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.add_module"
        and row.case_id == "ftmodapi.FT_MODULE_STYLER.styler_module_registration"
    ):
        return "FT_MODULE_STYLER registration validates stored styler module flags, FT_Get_Module lookup, private interface presence, module_init callback, and unchanged outline renderer routing through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.add_module"
        and row.case_id == "ftmodapi.FT_MODULE_RENDERER.renderer_module_registration"
    ):
        return "FT_MODULE_RENDERER registration validates renderer module insertion, FT_Get_Module lookup, stored renderer class fields, module_init callback, preserved default outline renderer lookup, and FT_Set_Renderer membership for the installed renderer through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.add_module"
        and row.case_id == "ftmodapi.FT_Add_Module.rejects_null_library"
        and lifecycle_handle(row, "library") == "null"
    ):
        return "FT_Add_Module null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.add_module"
        and row.case_id == "ftmodapi.FT_Add_Module.rejects_null_class"
        and lifecycle_handle(row, "module_class") == "null"
    ):
        return "FT_Add_Module null-class error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.add_module"
        and row.case_id == "ftmodapi.FT_Add_Module.rejects_future_required_version"
    ):
        return "FT_Add_Module future-required-version error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.add_module"
        and row.case_id == "ftmodapi.FT_Add_Module.duplicate_name_version_rules"
    ):
        return "FT_Add_Module duplicate-name/version replacement behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "FT_Add_Module"
        and row.case_id == "fterrdef.FT_Err_Invalid_Version.module_requires_newer_freetype"
    ):
        return "FT_Add_Module future-FreeType-version rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "FT_Add_Module"
        and row.case_id == "fterrdef.FT_Err_Lower_Module_Version.duplicate_module_not_newer"
    ):
        return "FT_Add_Module lower duplicate module-version rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "FT_Add_Module"
        and row.case_id == "fterrdef.FT_Err_Too_Many_Drivers.module_registry_limit"
    ):
        return "FT_Add_Module module-registry-limit rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.new_library"
        and row.case_id == "ftmodapi.FT_New_Library.rejects_null_inputs_preserving_output"
    ):
        return "FT_New_Library null-input error/output preservation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.new_library"
        and row.case_id == "ftmodapi.FT_New_Library.creates_library_with_version_and_refcount"
    ):
        return "FT_New_Library allocator-backed success, version, memory identity, refcount, and default-module absence validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.new_library"
        and row.case_id == "ftmodapi.FT_New_Library.allocation_failure_preserves_output"
    ):
        return "FT_New_Library allocator-failure output preservation validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.reference_library"
        and row.case_id == "ftmodapi.FT_Reference_Library.increments_refcount"
    ):
        return "FT_Reference_Library refcount increment and first-done usability validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.reference_then_done_library"
        and row.case_id == "ftmodapi.FT_Done_Library.decrements_reference_without_destroying"
    ):
        return "FT_Done_Library refcount decrement without destruction validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftincrem.open_face_without_incremental_parameter"
        and row.case_id
        == "ftincrem.FT_Incremental_InterfaceRec.absent_parameter_uses_embedded_data"
    ):
        return "Absent FT_PARAM_TAG_INCREMENTAL open/load behavior validates embedded font data use and zero callback count through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftincrem.open_face_incremental_nullness"
        and row.case_id
        == "ftincrem.FT_Incremental_Interface.null_or_absent_interface_behavior"
    ):
        return "Absent and NULL FT_PARAM_TAG_INCREMENTAL interface behavior validates embedded font data use, null stored-interface class, and zero callback count through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.open_face_incremental"
        and row.case_id
        == "ftparams.FT_PARAM_TAG_INCREMENTAL.missing_or_null_interface_matches_c"
    ):
        return "FT_PARAM_TAG_INCREMENTAL absent and NULL parameter-data behavior validates FT_Open_Face parameter dispatch, embedded font data use, null stored-interface class, and zero callback count through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.face_driver_name"
        and row.case_id == "ftmodapi.FT_FACE_DRIVER_NAME.returns_driver_module_name"
    ):
        return "FT_FACE_DRIVER_NAME TrueType driver module name validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.face_driver_name_with_font_format"
        and row.case_id == "ftmodapi.FT_FACE_DRIVER_NAME.driver_name_not_font_format"
    ):
        return "FT_FACE_DRIVER_NAME non-TrueType CFF driver module name and FT_Get_Font_Format service string validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.property_get"
        and row.case_id == "ftmodapi.FT_Property_Get.rejects_null_arguments"
    ):
        return "FT_Property_Get null-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "face_lifecycle.load_render_done"
        and row.case_id == "fterrdef.FT_Err_Ok.successful_face_lifecycle"
    ):
        return "FT_Err_Ok lifecycle success validates load/render glyph status plus exact slot metrics and bitmap output through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "representative_success_outputs"
        and row.case_id == "fterrdef.FT_Err_Ok.successful_constant_status_does_not_mask_output"
    ):
        return "FT_Err_Ok success control validates non-status glyph metrics and bitmap output through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.property_get"
        and row.case_id == "ftmodapi.FT_Property_Get.gets_supported_property"
    ):
        return "FT_Property_Get supported TrueType property value validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.property_get"
        and row.case_id == "ftmodapi.FT_Property_Get.missing_or_unsupported_property_service"
    ):
        return "FT_Property_Get missing-module and unsupported-service errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.property_get"
        and row.case_id == "ftmodapi.FT_Property_Get.invalid_property_name"
    ):
        return "FT_Property_Get invalid-property-name errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.property_set"
        and row.case_id == "ftmodapi.FT_Property_Set.rejects_null_arguments"
    ):
        return "FT_Property_Set null-argument errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.property_set_then_get"
        and row.case_id == "ftmodapi.FT_Property_Set.sets_supported_property"
    ):
        return "FT_Property_Set supported TrueType property round-trip validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.property_set"
        and row.case_id == "ftmodapi.FT_Property_Set.missing_or_unsupported_property_service"
    ):
        return "FT_Property_Set missing-module and unsupported-service errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.property_set"
        and row.case_id == "ftmodapi.FT_Property_Set.invalid_property_or_value"
    ):
        return "FT_Property_Set invalid-property-name/value errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_filter_weights"
        and row.case_id == "ftlcdfil.FT_Library_SetLcdFilterWeights.error_null_library"
    ):
        return "FT_Library_SetLcdFilterWeights null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_filter_weights"
        and row.case_id == "ftlcdfil.FT_Library_SetLcdFilterWeights.error_null_weights"
    ):
        return "FT_Library_SetLcdFilterWeights null-weights error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_geometry"
        and row.case_id == "ftlcdfil.FT_Library_SetLcdGeometry.error_null_library"
    ):
        return "FT_Library_SetLcdGeometry null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_geometry"
        and row.case_id == "ftlcdfil.FT_Library_SetLcdGeometry.error_null_geometry"
    ):
        return "FT_Library_SetLcdGeometry null-geometry error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_geometry"
        and row.case_id
        == "ftlcdfil.FT_Library_SetLcdGeometry.unimplemented_with_subpixel_filtering"
    ):
        return "FT_Library_SetLcdGeometry unavailable-subpixel error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_filter"
        and row.case_id == "ftlcdfil.FT_Library_SetLcdFilter.error_null_library"
    ):
        return "FT_Library_SetLcdFilter null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_filter"
        and row.case_id == "ftlcdfil.FT_Library_SetLcdFilter.error_invalid_filter"
    ):
        return "FT_Library_SetLcdFilter invalid-filter errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_filter"
        and row.case_id == "ftlcdfil.FT_LcdFilter.rejected_filter_values"
    ):
        return "FT_LcdFilter rejected-filter values validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftlcdfil.set_lcd_filter"
        and row.case_id
        in {
            "ftlcdfil.FT_LCD_FILTER_LEGACY.rejected_by_set_lcd_filter",
            "ftlcdfil.FT_LCD_FILTER_LEGACY1.rejected_by_set_lcd_filter",
            "ftlcdfil.FT_LCD_FILTER_MAX.rejected_by_set_lcd_filter",
        }
    ):
        return "FT_Library_SetLcdFilter enum rejection validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_var_design_coordinates"
        and row.case_id == "ftmm.FT_Get_Var_Design_Coordinates.error_null_coords"
    ):
        return "FT_Get_Var_Design_Coordinates null-coords error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_var_design_coordinates"
        and row.case_id
        == "ftmm.FT_Get_Var_Design_Coordinates.success_after_set_var_design_coordinates"
    ):
        return "FT_Get_Var_Design_Coordinates after FT_Set_Var_Design_Coordinates validates active OpenType design coordinates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_var_blend_coordinates"
        and row.case_id == "ftmm.FT_Get_Var_Blend_Coordinates.error_null_coords"
    ):
        return "FT_Get_Var_Blend_Coordinates null-coords error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_var_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Get_Var_Blend_Coordinates.success_after_set_var_blend_coordinates"
    ):
        return "FT_Get_Var_Blend_Coordinates after FT_Set_Var_Blend_Coordinates validates active OpenType normalized coordinates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_var_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Set_Var_Blend_Coordinates.success_partial_extra_and_reset"
    ):
        return "FT_Set_Var_Blend_Coordinates partial, excess, and reset scenarios validate active OpenType design/blend coordinate state through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_var_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Set_Var_Blend_Coordinates.output_changes_for_active_blend"
    ):
        return "FT_Set_Var_Blend_Coordinates glyph output validates active blend coordinates followed by FT_Load_Glyph/FT_Render_Glyph through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_var_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_Var_Design_Coordinates.success_partial_extra_and_reset"
    ):
        return "FT_Set_Var_Design_Coordinates partial, excess, and reset scenarios validate active OpenType design/blend coordinate state through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_var_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_Var_Design_Coordinates.success_updates_metrics_variations"
    ):
        return "FT_Set_Var_Design_Coordinates metrics variation validates face metrics, size metrics, and glyph advance after design-coordinate mutation through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_var_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_Var_Design_Coordinates.output_changes_for_design_coordinates"
    ):
        return "FT_Set_Var_Design_Coordinates glyph output validates set design coordinates followed by FT_Load_Glyph/FT_Render_Glyph through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_var_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Get_Var_Blend_Coordinates.error_non_variable_or_invalid_face"
    ):
        return "FT_Get_Var_Blend_Coordinates invalid/non-variable-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_blend_coordinates"
        and row.case_id == "ftmm.FT_Get_MM_Blend_Coordinates.invalid_face_or_coords_error"
    ):
        return "FT_Get_MM_Blend_Coordinates invalid-face/null-coords errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_Get_MM_Var.null_output_error"
    ):
        return "FT_Get_MM_Var null-output error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_Get_MM_Var.invalid_or_non_variable_face_error"
    ):
        return "FT_Get_MM_Var invalid/non-variable-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_Get_MM_Var.variable_font_descriptor_success"
    ):
        return "FT_Get_MM_Var OpenType variable descriptor validates fvar axes, namedstyles, axis flags, and FT_Done_MM_Var release through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_multi_master"
        and row.case_id == "ftmm.FT_Multi_Master.populated_by_adobe_mm_service"
    ):
        return "FT_Get_Multi_Master validates Adobe MM descriptor population plus OpenType variable-face error control through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_Var_Named_Style.coordinates_array_matches_axis_count"
    ):
        return "FT_Var_Named_Style coordinate arrays validate one coordinate per axis through OpenType FT_Get_MM_Var across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_Var_Named_Style.psid_missing_sentinel_matches_c"
    ):
        return "FT_Var_Named_Style missing PostScript-name sentinel validates as 0xFFFF through OpenType FT_Get_MM_Var across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_Get_MM_Var.adobe_mm_descriptor_success"
    ):
        return "FT_Get_MM_Var Adobe MM descriptor allocation validates through the generated Type 1 MM fixture, pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_MM_Var.populated_for_adobe_mm"
    ):
        return "FT_MM_Var Adobe MM record fields validate through FT_Get_MM_Var on the generated Type 1 MM fixture across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_Var_Axis.adobe_mm_axis_values"
    ):
        return "FT_Var_Axis Adobe MM axis fields validate through FT_Get_MM_Var on the generated Type 1 MM fixture across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var"
        and row.case_id == "ftmm.FT_Var_Axis.variable_font_axis_values"
    ):
        return "FT_Var_Axis OpenType wght/wdth/opsz axis fields validate through the generated three-axis fvar fixture across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var_then_axis_flags"
        and row.case_id == "ftmm.FT_Get_Var_Axis_Flags.valid_axis_flags"
    ):
        return "FT_Get_Var_Axis_Flags Adobe MM zero axis flags validate through FT_Get_MM_Var on the generated Type 1 MM fixture across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var_then_axis_flags"
        and row.case_id == "ftmm.FT_Get_Var_Axis_Flags.hidden_axis_flag"
    ):
        return "FT_Get_Var_Axis_Flags hidden OpenType fvar axis flags validate through the generated hidden-axis fixture across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_var_then_axis_flags"
        and row.case_id == "ftmm.FT_VAR_AXIS_FLAG_HIDDEN.returned_by_axis_flags"
    ):
        return "FT_VAR_AXIS_FLAG_HIDDEN visible/hidden flag rows validate through generated OpenType fvar fixtures across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_var_axis_flags"
        and row.case_id == "ftmm.FT_Var_Axis.hidden_axis_flag_adjacent_storage"
    ):
        return "FT_Var_Axis adjacent hidden-axis flag storage validates through FT_Get_Var_Axis_Flags on the generated named-instance hidden-axis fixture across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_weightvector"
        and row.case_id == "ftmm.FT_Get_MM_WeightVector.len_without_buffer_error"
    ):
        return "FT_Get_MM_WeightVector len-without-buffer error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_weightvector"
        and row.case_id == "ftmm.FT_Get_MM_WeightVector.unsupported_face_error"
    ):
        return "FT_Get_MM_WeightVector unsupported-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_mm_weightvector"
        and row.case_id == "ftmm.FT_Get_MM_WeightVector.adobe_mm_weightvector_success"
    ):
        return "FT_Get_MM_WeightVector Adobe MM capacity matrix validates through the declared generated Type 1 MM fixture, pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_multi_master"
        and row.case_id == "ftmm.FT_MM_Axis.populated_by_get_multi_master"
    ):
        return "FT_MM_Axis populated slots validate through FT_Get_Multi_Master across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_multi_master"
        and row.case_id == "ftmm.FT_Get_Multi_Master.adobe_mm_descriptor_success"
    ):
        return "FT_Get_Multi_Master Adobe MM descriptor success validates through the declared generated Type 1 MM fixture, pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_multi_master"
        and row.case_id == "ftmm.T1_MAX_MM_DESIGNS.record_design_capacity"
    ):
        return "T1_MAX_MM_DESIGNS record design capacity validates via FT_Get_Multi_Master on the declared generated Type 1 MM fixture across pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_multi_master"
        and row.case_id
        == "ftmm.FT_Get_Multi_Master.true_type_or_opentype_variation_error"
    ):
        return "FT_Get_Multi_Master TrueType/OpenType variation errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.get_multi_master"
        and row.case_id == "ftmm.FT_Get_Multi_Master.invalid_or_non_variable_face_error"
    ):
        return "FT_Get_Multi_Master invalid/non-variable-face errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Design_Coordinates.error_non_adobe_variation_face"
    ):
        return "FT_Set_MM_Design_Coordinates non-Adobe/non-MM variation errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_weight_vector"
        and row.case_id
        == "ftmm.FT_Set_MM_WeightVector.error_null_weightvector_with_nonzero_len"
    ):
        return "FT_Set_MM_WeightVector null-weightvector error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_weight_vector"
        and row.case_id == "ftmm.FT_Set_MM_WeightVector.success_set_weight_vector"
    ):
        return "FT_Set_MM_WeightVector success validates Type 1 MM weight vector state through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Design_Coordinates.success_adobe_mm_design_coordinates"
    ):
        return "FT_Set_MM_Design_Coordinates validates generated Type 1 MM design-coordinate state and follow-up design/blend getters through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Design_Coordinates.success_partial_extra_and_reset"
    ):
        return "FT_Set_MM_Design_Coordinates partial, extra, and reset scenarios validate generated Type 1 MM design-coordinate state through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Design_Coordinates.output_changes_for_mm_design_loadable_glyph"
    ):
        return "FT_Set_MM_Design_Coordinates loadable Type 1 MM glyph output validates design-coordinate mutation through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_named_instance"
        and row.case_id == "ftmm.FT_Set_Named_Instance.success_adobe_mm_resets_default"
    ):
        return "FT_Set_Named_Instance(0) validates Adobe MM reset-to-default state after Type 1 MM design-coordinate mutation through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_weight_vector"
        and row.case_id == "ftmm.FT_Set_MM_WeightVector.success_short_long_and_reset"
    ):
        return "FT_Set_MM_WeightVector short, long, and reset scenarios validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_weight_vector"
        and row.case_id == "ftmm.FT_Set_MM_WeightVector.success_unenforced_weight_sum"
    ):
        return "FT_Set_MM_WeightVector unenforced weight sum behavior validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_weight_vector"
        and row.case_id
        == "ftmm.FT_Set_MM_WeightVector.error_unsupported_on_true_type_variations"
    ):
        return "FT_Set_MM_WeightVector unsupported TrueType variation errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_var_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_Var_Design_Coordinates.error_null_coords_with_nonzero_count"
    ):
        return "FT_Set_Var_Design_Coordinates null-coords error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_var_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Set_Var_Blend_Coordinates.error_null_coords_with_nonzero_count"
    ):
        return "FT_Set_Var_Blend_Coordinates null-coords error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Blend_Coordinates.error_null_coords_with_nonzero_count"
    ):
        return "FT_Set_MM_Blend_Coordinates null-coords error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Blend_Coordinates.success_set_normalized_coordinates"
    ):
        return "FT_Set_MM_Blend_Coordinates Adobe MM normalized-coordinate success validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Blend_Coordinates.success_partial_and_extra_coordinates"
    ):
        return "FT_Set_MM_Blend_Coordinates Adobe MM partial and extra coordinate scenarios validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_blend_coordinates"
        and row.case_id == "ftmm.FT_Set_MM_Blend_Coordinates.success_reset_to_default"
    ):
        return "FT_Set_MM_Blend_Coordinates Adobe MM reset-to-default validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Blend_Coordinates.success_type1_mm_glyph_output_after_blend"
    ):
        return "FT_Set_MM_Blend_Coordinates Type 1 MM glyph output after active blend validates FT_Load_Glyph/FT_Render_Glyph through pinned C oracle, Rust FFI, C ABI, and WASM ABI; the separate blend-dependent output-change row remains pending"
    if (
        row.operation == "ftmm.set_then_get_mm_blend_coordinates"
        and row.case_id
        == "ftmm.FT_Get_MM_Blend_Coordinates.after_set_blend_coordinates"
    ):
        return "FT_Get_MM_Blend_Coordinates after FT_Set_Var_Blend_Coordinates validates active normalized blend state through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.set_mm_design_coordinates"
        and row.case_id
        == "ftmm.FT_Set_MM_Design_Coordinates.error_null_coords_with_nonzero_count"
    ):
        return "FT_Set_MM_Design_Coordinates null-coords error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.load_target_mode"
        and row.case_id == "freetype.FT_RENDER_MODE_NORMAL.maps_supported_modes"
    ):
        return "FT_RENDER_MODE_NORMAL to FT_LOAD_TARGET_NORMAL roundtrip validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.open_face_pair"
        and row.case_id == "freetype.FT_STYLE_FLAG_BOLD.face_style_flag_behavior"
    ):
        return "FT_STYLE_FLAG_BOLD face-pair style flags validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.face_flags_after_variation"
        and row.case_id == "freetype.FT_FACE_FLAG_VARIATION.face_property_variation_selection"
    ):
        return "FT_FACE_FLAG_VARIATION set and declared named-instance reset probe validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.open_face_stream"
        and row.case_id == "freetype.FT_FACE_FLAG_EXTERNAL_STREAM.open_face_stream_ownership"
    ):
        return "FT_OPEN_STREAM external-stream ownership validates face_flags, external-stream bit, close-call count, and caller stream lifetime through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.open_face_stream"
        and row.case_id
        in {
            "freetype.FT_Open_Args.stream_source_success_matches_c",
            "ftsystem.FT_Stream.valid_external_memory_stream_face_open",
            "ftsystem.FT_StreamRec.external_base_close_fields_match_c",
        }
    ):
        return "FT_OPEN_STREAM valid caller-owned external memory stream validates return code, opened flag, external-stream face flag, close-call count, and caller stream lifetime through pinned C oracle, Rust FFI, C ABI, and WASM ABI; malformed callback stream harness rows remain pending"
    if (
        row.operation == "freetype.active_size_handle"
        and row.case_id == "freetype.FT_Size.active_size_handle_runtime"
    ):
        return "FT_Size active face size handle, metrics, and follow-up glyph-load scaling validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.face_owned_handles"
        and row.case_id == "freetype.FT_Face.owns_slot_size_and_charmaps"
    ):
        return "FT_Face-owned glyph, active size, and charmap handle ownership classes validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI without raw-address comparison"
    if (
        row.operation == "ftmodapi.remove_module"
        and row.case_id == "ftmodapi.FT_Remove_Module.rejects_null_library"
        and lifecycle_handle(row, "library") == "null"
    ):
        return "FT_Remove_Module null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmodapi.remove_module"
        and row.case_id == "ftmodapi.FT_Remove_Module.rejects_missing_or_foreign_module"
    ):
        return "FT_Remove_Module missing/foreign-module errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "ftmm.done_mm_var"
        and row.case_id == "ftmm.FT_Done_MM_Var.null_library_error"
        and param_is_null(row, "library")
    ):
        return "FT_Done_MM_Var null-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.done_face"
        and row.case_id == "freetype.FT_Done_Face.error_null_face"
        and lifecycle_handle(row, "face") == "null"
    ):
        return "FT_Done_Face null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_char"
        and row.case_id
        == "freetype.FT_Load_Char.error_null_face_or_invalid_flags.null_face"
        and lifecycle_handle(row, "face") == "null"
    ):
        return "FT_Load_Char null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_char"
        and row.case_id == "freetype.FT_Load_Char.error_null_face_or_invalid_flags"
        and row.case == "error_null_face_or_invalid_flags"
    ):
        return "FT_Load_Char reserved-load-flag error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_char_size"
        and row.case_id == "freetype.FT_Set_Char_Size.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Set_Char_Size null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_char_size"
        and row.case_id == "freetype.FT_Set_Char_Size.error_probe_face_invalid_size_handle"
    ):
        return "FT_Set_Char_Size probe-face invalid-size-handle error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_pixel_sizes"
        and row.case_id == "freetype.FT_Set_Pixel_Sizes.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Set_Pixel_Sizes null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "set_pixel_sizes"
        and row.case_id
        == "freetype.FT_Set_Pixel_Sizes.error_probe_face_invalid_size_handle"
    ):
        return "FT_Set_Pixel_Sizes probe-face invalid-size-handle error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_charmap"
        and row.case_id == "freetype.FT_Select_Charmap.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Select_Charmap null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_charmap"
        and row.case_id == "freetype.FT_Select_Charmap.error_missing_encoding"
    ):
        return "FT_Select_Charmap missing-encoding error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_charmap"
        and row.case_id == "freetype.FT_Select_Charmap.error_missing_unicode_charmap"
    ):
        return "FT_Select_Charmap missing-Unicode-charmap error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.set_charmap"
        and row.case_id == "freetype.FT_Set_Charmap.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Set_Charmap null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.set_charmap"
        and row.case_id == "freetype.FT_Set_Charmap.error_null_or_foreign_charmap"
    ):
        return "FT_Set_Charmap null/foreign-charmap errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.set_charmap"
        and row.case_id == "freetype.FT_Set_Charmap.error_format14_charmap"
    ):
        return "FT_Set_Charmap format-14-charmap error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.set_charmap"
        and row.case_id
        == "fterrdef.FT_Err_Invalid_CharMap_Handle.set_charmap_rejects_foreign_or_null_charmap"
    ):
        return "FT_Err_Invalid_CharMap_Handle set-charmap null-charmap error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_size"
        and row.case_id == "freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Select_Size null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_size"
        and row.case_id == "freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face"
        and not lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Select_Size no-fixed-sizes error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.reference_face"
        and row.case_id == "freetype.FT_Reference_Face.error_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Reference_Face null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_size"
        and row.case_id == "freetype.FT_Select_Size.error_strike_index_past_end_direct"
    ):
        return "FT_Select_Size past-end strike-index error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.select_size"
        and row.case_id == "freetype.FT_Select_Size.error_strike_index_out_of_range"
    ):
        return "FT_Select_Size strike-index range errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id == "freetype.FT_Request_Size.error_null_face_or_request"
        and any(
            isinstance(variant, dict)
            and (variant.get("face") == "null" or variant.get("request") == "null")
            for variant in list_value(row.params.get("variants"))
        )
    ):
        return "FT_Request_Size null-face/null-request errors validate through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id == "freetype.FT_Request_Size.error_ppem_overflow"
    ):
        return "FT_Request_Size ppem-overflow error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id
        == "freetype.FT_SIZE_REQUEST_TYPE_MAX.request_size_rejects_sentinel"
    ):
        return "FT_Request_Size sentinel request-type error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id
        == "fterrdef.FT_Err_Divide_By_Zero.invalid_size_transform_division_returns_error"
    ):
        return "FT_Request_Size BBOX divide-by-zero error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id == "freetype.FT_Request_Size.error_invalid_request_or_unavailable_strike"
    ):
        return "FT_Request_Size invalid-request matrix validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "freetype.request_size"
        and row.case_id == "freetype.FT_Request_Size.error_probe_face_invalid_size_handle"
    ):
        return "FT_Request_Size probe-face invalid-size-handle error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id == "freetype.FT_Load_Glyph.error_null_face_or_invalid_flags.null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Load_Glyph null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    if (
        row.operation == "load_glyph"
        and row.case_id == "fterrdef.FT_Err_Invalid_Face_Handle.face_api_rejects_null_face"
        and lifecycle_handle_is_null(row, "face")
    ):
        return "FT_Err_Invalid_Face_Handle load-glyph null-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI"
    return None


def wrapper_null_validation_reason(row: ConcreteInput) -> str | None:
    if row.operation == "freetype.get_subglyph_info" and "null_output_indices" in row.params:
        return (
            "FT_Get_SubGlyph_Info native C dereferences valid-slot output pointers; "
            "the row validates Rust FFI, C ABI, and WASM ABI null-output policy "
            "after proving the subglyph is native-C-callable with non-null outputs"
        )
    return None


def raw_slot_null_validation_reason(row: ConcreteInput) -> str | None:
    if row.operation == "ftsynth.glyphslot_null_noop":
        return (
            "raw FT_GlyphSlot null no-op validates pinned C, Rust FFI, C ABI, "
            "and WASM ABI via the explicit null handle route"
        )
    return None


def null_error_real_parity_reason(row: ConcreteInput) -> str | None:
    reasons = {
        "freetype.FT_Done_Face.error_invalid_or_foreign_face_handle": "FT_Done_Face invalid/foreign-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_Done_FreeType.error_invalid_or_foreign_library_handle": "FT_Done_FreeType invalid/foreign-library error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_New_Face.error_null_pathname": "FT_New_Face null-pathname error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_Render_Glyph.error_null_or_unowned_slot": "FT_Render_Glyph null/unowned-slot error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_Set_Char_Size.error_invalid_or_unscalable_face": "FT_Set_Char_Size invalid/unscalable-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "freetype.FT_Set_Pixel_Sizes.error_invalid_or_unscalable_face": "FT_Set_Pixel_Sizes invalid/unscalable-face error validates through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
    }
    return reasons.get(row.case_id)


def bdf_property_exact_error_real_parity_reason(row: ConcreteInput) -> str | None:
    reasons = {
        "ftbdf.FT_Get_BDF_Property.error_missing_property_sets_none": "FT_Get_BDF_Property missing-property error validates exact status and BDF_PropertyRec NONE/sentinel state through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftbdf.FT_Get_BDF_Property.error_null_face_or_output": "FT_Get_BDF_Property null-face/null-output errors validate exact status and output preservation through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
        "ftbdf.FT_Get_BDF_Property.error_unsupported_face_or_unselected_strike": "FT_Get_BDF_Property unsupported-face error validates exact status and BDF_PropertyRec NONE/sentinel state through pinned C oracle, Rust FFI, C ABI, and WASM ABI",
    }
    return reasons.get(row.case_id)


def list_value(value: object) -> list[object]:
    return value if isinstance(value, list) else []


def shape_fallback_reason(row: ConcreteInput) -> str | None:
    params = row.params
    operation = row.operation
    if (
        row.case_id
        == "freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format.unrouted_slot_states"
    ):
        return None
    if operation == "load_char" and "char_code" not in params:
        return "load_char lacks char_code"
    if operation == "load_glyph" and "glyph_index" not in params and "glyph_selector" not in params:
        return "load_glyph lacks glyph selector"
    if operation == "render_glyph" and not any(
        key in params for key in ("char_code", "glyph_index", "glyph_selector")
    ):
        return "render_glyph lacks glyph selector"
    if operation == "set_char_size" and "variants" in params:
        return "set_char_size variants are not routed through the real oracle"
    if (
        operation == "freetype.request_size"
        and "requests" not in params
        and "request" not in params
        and "variants" not in params
    ):
        return "request_size lacks request rows"
    if operation == "freetype.set_charmap" and not any(
        key in params for key in ("charmap_indices", "variants")
    ):
        return "set_charmap lacks charmap selector rows"
    if operation == "ftoutln.outline_get_cbox" and not any(
        key in params for key in ("glyph_index", "glyph_indices", "scenarios")
    ):
        return "outline_get_cbox lacks glyph selector"
    if operation == "ftsnames.get_sfnt_name" and "indexes" not in params:
        return "get_sfnt_name lacks name indexes"
    if operation.startswith("sfnt.get_sfnt_table") and any(
        key in params for key in ("variation_sequence", "variation_calls")
    ):
        if (
            row.case_id
            == "tttables.TT_VertHeader.sfnt_table_present_runtime.mvar_variation"
        ):
            return None
        return "sfnt table variation sequence is not routed"
    if operation == "sfnt.load_sfnt_table" and not any(
        key in params for key in ("offset", "reads", "tags")
    ):
        return "load_sfnt_table lacks offset/read/tag selector"
    if operation == "sfnt.table_info" and not any(
        key in params
        for key in ("table_index", "invalid_index", "table_indices", "table_index_ignored")
    ):
        return "table_info lacks table index selector"
    if operation == "freetype.new_face" and any(
        lifecycle_handle(row, name) == "null" for name in ("pathname", "library", "aface")
    ):
        return "new_face null-handle row uses generic oracle fallback"
    if (
        operation == "new_memory_face"
        and not has_runtime_asset(row)
        and lifecycle_handle(row, "file_base") != "null"
    ):
        return "new_memory_face lacks runtime font asset"
    if operation == "set_pixel_sizes" and not has_runtime_asset(row):
        if lifecycle_handle(row, "face") != "null":
            return f"{operation} lacks runtime font asset"
    if operation in {"freetype.done_freetype", "freetype.done_face"} and not has_runtime_asset(row):
        handle_name = "library" if operation == "freetype.done_freetype" else "face"
        if lifecycle_handle(row, handle_name) != "null":
            return f"{operation} lacks runtime font asset"
    if operation == "freetype.face_set_unpatented_hinting" and not any(
        key in params for key in ("bool_values", "values", "value", "toggle_sequence")
    ):
        return "face_set_unpatented_hinting lacks bool values"
    if operation in {
        "ftsynth.glyphslot_slant_noop",
        "ftsynth.glyphslot_oblique_noop",
        "ftsynth.glyphslot_adjust_weight_noop",
        "ftsynth.glyphslot_embolden_noop",
    }:
        return "synthetic unsupported glyph-slot fixture is required_future_asset"
    if (
        operation
        in {
            "ftsynth.glyphslot_adjust_weight_after_load",
            "ftsynth.glyphslot_embolden_after_load",
        }
        and "bitmap_strike_font" in row.assets
    ):
        return "embedded bitmap strike fixture is required_future_asset"
    return None


def unresolved_runtime_asset_pending_reason(row: ConcreteInput) -> str | None:
    runtime_skipped_needs_input_cases = {
        "ftcid.FT_Get_CID_From_Glyph_Index.cid_face_returns_cid",
        "ftcid.FT_Get_CID_From_Glyph_Index.null_cid_output_matches_c",
        # SFNT-wrapped CID rows are backed by the maintained OFL-1.1
        # FDArrayTest257 OpenType/CFF CID fixture.
        "ftcid.FT_Get_CID_Is_Internally_CID_Keyed.cid_face_reports_true",
        "ftcid.FT_Get_CID_Is_Internally_CID_Keyed.null_output_matches_c",
        # SFNT-wrapped CID keyed-state row is backed by the maintained
        # FDArrayTest257 OpenType/CFF CID fixture.
        "ftgxval.FT_VALIDATE_GX.validates_all_requested_tables",
        "ftgxval.FT_VALIDATE_GX_LENGTH.controls_output_slot_initialization",
        "ftgxval.FT_VALIDATE_MS.validates_ms_classic_kern",
        "ftgxval.FT_VALIDATE_bsln.validates_bsln_table_slot",
        "ftgxval.FT_VALIDATE_bsln_INDEX.indexes_bsln_output_slot",
        "ftgxval.FT_VALIDATE_feat.validates_feat_table_slot",
        "ftgxval.FT_VALIDATE_feat_INDEX.indexes_feat_output_slot",
        "ftgxval.FT_VALIDATE_just.validates_just_table_slot",
        "ftgxval.FT_VALIDATE_just_INDEX.indexes_just_output_slot",
        "ftgxval.FT_VALIDATE_kern.validates_gx_kern_table_slot",
        "ftgxval.FT_VALIDATE_kern_INDEX.indexes_kern_output_slot",
        "ftgxval.FT_VALIDATE_lcar.validates_lcar_table_slot",
        "ftgxval.FT_VALIDATE_lcar_INDEX.indexes_lcar_output_slot",
        "ftgxval.FT_VALIDATE_mort.validates_mort_table_slot",
        "ftgxval.FT_VALIDATE_mort_INDEX.indexes_mort_output_slot",
        "ftgxval.FT_VALIDATE_morx.validates_morx_table_slot",
        "ftgxval.FT_VALIDATE_morx_INDEX.indexes_morx_output_slot",
        "ftpfr.FT_Get_PFR_Advance.pfr_glyph_advance_success",
        "ftpfr.FT_Get_PFR_Kerning.pfr_pair_kerning_success",
        "ttnameid.TT_ADOBE_ID_CUSTOM.representative_charmap_encoding_match",
        "ttnameid.TT_PLATFORM_ADOBE.representative_adobe_charmap_match",
    }
    if row.case_id in runtime_skipped_needs_input_cases:
        return (
            "declared semantic row has no maintained runtime-resolved input; "
            "exact runtime parity requires the declared same input to execute "
            "against pinned C, Rust FFI, thin C ABI, and WASM ABI; counting the "
            "selection-skipped row as real parity would be a green placeholder"
        )
    if row.expect_error or exact_error_public_route(
        row.operation, row.case_id, row.expect_error
    ):
        return None
    if not operation_is_real_parity(row.operation):
        return None
    unresolved = unresolved_assets_reason(row)
    if not unresolved:
        return None
    return (
        f"{unresolved}; exact runtime parity requires the declared same input "
        "to resolve for pinned C, Rust FFI, thin C ABI, and WASM ABI; counting "
        "a different input or skipped runtime case as real parity would be a "
        "green placeholder"
    )


def route_category(row: ConcreteInput) -> tuple[str, str]:
    pending = pending_core_reason(row)
    if pending:
        return ("pending-core", pending)
    bdf_route_pending = pending_route_reason(row)
    if bdf_route_pending and row.operation in {
        "ftbdf.get_bdf_charset_id",
        "ftbdf.get_bdf_property",
    }:
        return ("pending-route", bdf_route_pending)
    otvalid_unresolved_pending = otvalid_unresolved_asset_pending_reason(row)
    if otvalid_unresolved_pending:
        return ("pending-route", otvalid_unresolved_pending)
    property_pending = property_service_pending_reason(row)
    if property_pending:
        return ("pending-route", property_pending)
    null_error_real_reason = null_error_real_parity_reason(row)
    if null_error_real_reason:
        return ("real-parity", null_error_real_reason)
    bdf_property_exact_error_reason = bdf_property_exact_error_real_parity_reason(row)
    if bdf_property_exact_error_reason:
        return ("real-parity", bdf_property_exact_error_reason)
    done_glyph_pending = done_glyph_lifecycle_pending_reason(row)
    if done_glyph_pending:
        return ("pending-route", done_glyph_pending)
    ftstroke_null_noop_reason = ftstroke_null_noop_real_parity_reason(row)
    if ftstroke_null_noop_reason:
        return ("real-parity", ftstroke_null_noop_reason)
    ftstroke_zero_line_reason = ftstroke_zero_line_real_parity_reason(row)
    if ftstroke_zero_line_reason:
        return ("real-parity", ftstroke_zero_line_reason)
    ftstroke_degenerate_curve_reason = ftstroke_degenerate_curve_real_parity_reason(row)
    if ftstroke_degenerate_curve_reason:
        return ("real-parity", ftstroke_degenerate_curve_reason)
    ftstroke_parse_line_conic_cubic_reason = ftstroke_parse_line_conic_cubic_real_parity_reason(row)
    if ftstroke_parse_line_conic_cubic_reason:
        return ("real-parity", ftstroke_parse_line_conic_cubic_reason)
    ftstroke_set_attribute_geometry_reason = ftstroke_set_attribute_geometry_real_parity_reason(row)
    if ftstroke_set_attribute_geometry_reason:
        return ("real-parity", ftstroke_set_attribute_geometry_reason)
    ftstroke_parse_degenerate_reason = ftstroke_parse_degenerate_real_parity_reason(row)
    if ftstroke_parse_degenerate_reason:
        return ("real-parity", ftstroke_parse_degenerate_reason)
    ftstroke_end_no_segment_reason = ftstroke_end_no_segment_real_parity_reason(row)
    if ftstroke_end_no_segment_reason:
        return ("real-parity", ftstroke_end_no_segment_reason)
    ftstroke_finalized_counts_reason = ftstroke_finalized_counts_real_parity_reason(row)
    if ftstroke_finalized_counts_reason:
        return ("real-parity", ftstroke_finalized_counts_reason)
    ftstroke_reset_counts_reason = ftstroke_reset_counts_real_parity_reason(row)
    if ftstroke_reset_counts_reason:
        return ("real-parity", ftstroke_reset_counts_reason)
    ftstroke_rewind_attributes_reason = ftstroke_rewind_attributes_real_parity_reason(row)
    if ftstroke_rewind_attributes_reason:
        return ("real-parity", ftstroke_rewind_attributes_reason)
    ftstroke_set_miter_limit_reason = ftstroke_set_miter_limit_real_parity_reason(row)
    if ftstroke_set_miter_limit_reason:
        return ("real-parity", ftstroke_set_miter_limit_reason)
    ftstroke_miter_join_reason = ftstroke_miter_join_real_parity_reason(row)
    if ftstroke_miter_join_reason:
        return ("real-parity", ftstroke_miter_join_reason)
    absent_or_noop_reason = absent_or_noop_surface_real_parity_reason(row)
    if absent_or_noop_reason:
        return ("real-parity", absent_or_noop_reason)
    header_or_layout_reason = header_or_layout_compile_contract_reason(row)
    if header_or_layout_reason:
        return ("compile-contract", header_or_layout_reason)
    unresolved_asset_pending = unresolved_runtime_asset_pending_reason(row)
    if unresolved_asset_pending:
        return ("pending-route", unresolved_asset_pending)
    lifecycle_null_reason = lifecycle_null_real_parity_reason(row)
    if lifecycle_null_reason:
        return ("real-parity", lifecycle_null_reason)
    inspect_module_flags_real_reason = inspect_module_flags_real_parity_reason(row)
    if inspect_module_flags_real_reason:
        return ("real-parity", inspect_module_flags_real_reason)
    get_module_real_reason = get_module_real_parity_reason(row)
    if get_module_real_reason:
        return ("real-parity", get_module_real_reason)
    interpreter_version_property_real_reason = interpreter_version_property_real_parity_reason(row)
    if interpreter_version_property_real_reason:
        return ("real-parity", interpreter_version_property_real_reason)
    future_batch_pending = future_batch_unresolved_asset_pending_reason(row)
    if future_batch_pending:
        return ("pending-route", future_batch_pending)
    future_batch_real_reason = future_batch_real_parity_reason(row)
    if future_batch_real_reason:
        return ("real-parity", future_batch_real_reason)
    focused_success_real_reason = focused_success_real_parity_reason(row)
    if focused_success_real_reason:
        return ("real-parity", focused_success_real_reason)
    done_mm_var_real_reason = done_mm_var_real_parity_reason(row)
    if done_mm_var_real_reason:
        return ("real-parity", done_mm_var_real_reason)
    ftstroke_pending = ftstroke_stroker_pending_reason(row)
    if ftstroke_pending:
        return ("pending-route", ftstroke_pending)
    ftcache_pending = ftcache_subsystem_pending_reason(row)
    if ftcache_pending:
        return ("pending-route", ftcache_pending)
    ftcolor_colrv1_real = ftcolor_colrv1_composite_real_parity_reason(row)
    if ftcolor_colrv1_real:
        return ("real-parity", ftcolor_colrv1_real)
    ftcolor_pending = ftcolor_subsystem_pending_reason(row)
    if ftcolor_pending:
        return ("pending-route", ftcolor_pending)
    t1tables_pending = t1tables_subsystem_pending_reason(row)
    if t1tables_pending:
        return ("pending-route", t1tables_pending)
    ftgxval_pending = ftgxval_subsystem_pending_reason(row)
    if ftgxval_pending:
        return ("pending-route", ftgxval_pending)
    shape_reason = shape_fallback_reason(row)
    if shape_reason:
        if row.expect_error and not has_runtime_asset(row):
            return ("null-error-fallback", shape_reason)
        return ("shape-incomplete-fallback", shape_reason)
    if row.operation in EXPLICIT_UNSUPPORTED_OPERATIONS:
        return ("explicit-unsupported", "explicit Rust stub returns Unimplemented_Feature")
    if operation_is_compile_contract(row.operation):
        return ("compile-contract", "header, layout, macro, or scalar contract")
    route_pending = pending_route_reason(row)
    if route_pending:
        return ("pending-route", route_pending)
    property_pending = property_service_pending_reason(row)
    if property_pending:
        return ("pending-route", property_pending)
    lifecycle_null_reason = lifecycle_null_real_parity_reason(row)
    if lifecycle_null_reason:
        return ("real-parity", lifecycle_null_reason)
    inspect_module_flags_real_reason = inspect_module_flags_real_parity_reason(row)
    if inspect_module_flags_real_reason:
        return ("real-parity", inspect_module_flags_real_reason)
    get_module_real_reason = get_module_real_parity_reason(row)
    if get_module_real_reason:
        return ("real-parity", get_module_real_reason)
    interpreter_version_property_real_reason = interpreter_version_property_real_parity_reason(row)
    if interpreter_version_property_real_reason:
        return ("real-parity", interpreter_version_property_real_reason)
    future_batch_pending = future_batch_unresolved_asset_pending_reason(row)
    if future_batch_pending:
        return ("pending-route", future_batch_pending)
    future_batch_real_reason = future_batch_real_parity_reason(row)
    if future_batch_real_reason:
        return ("real-parity", future_batch_real_reason)
    ftmm_pending = ftmm_subsystem_pending_reason(row)
    if ftmm_pending:
        return ("pending-route", ftmm_pending)
    ftmodapi_pending = ftmodapi_subsystem_pending_reason(row)
    if ftmodapi_pending:
        return ("pending-route", ftmodapi_pending)
    ftdriver_pending = ftdriver_subsystem_pending_reason(row)
    if ftdriver_pending:
        return ("pending-route", ftdriver_pending)
    ftincrem_pending = ftincrem_subsystem_pending_reason(row)
    if ftincrem_pending:
        return ("pending-route", ftincrem_pending)
    ftglyph_pending = ftglyph_subsystem_pending_reason(row)
    if ftglyph_pending:
        return ("pending-route", ftglyph_pending)
    ftparams_name_option_reason = ftparams_name_option_real_parity_reason(row)
    if ftparams_name_option_reason:
        return ("real-parity", ftparams_name_option_reason)
    ftparams_ignored_param_reason = ftparams_ignored_param_real_parity_reason(row)
    if ftparams_ignored_param_reason:
        return ("real-parity", ftparams_ignored_param_reason)
    ftparams_pending = ftparams_subsystem_pending_reason(row)
    if ftparams_pending:
        return ("pending-route", ftparams_pending)
    ftimage_pending = ftimage_subsystem_pending_reason(row)
    if ftimage_pending:
        return ("pending-route", ftimage_pending)
    freetype_core_pending = freetype_core_subsystem_pending_reason(row)
    if freetype_core_pending:
        return ("pending-route", freetype_core_pending)
    specialized_record_pending = specialized_record_subsystem_pending_reason(row)
    if specialized_record_pending:
        return ("pending-route", specialized_record_pending)
    stream_pending = stream_subsystem_pending_reason(row)
    if stream_pending:
        return ("pending-route", stream_pending)
    callback_provider_pending = callback_provider_subsystem_pending_reason(row)
    if callback_provider_pending:
        return ("pending-route", callback_provider_pending)
    malformed_maxp_fixture_pending = malformed_maxp_fixture_pending_reason(row)
    if malformed_maxp_fixture_pending:
        return ("pending-route", malformed_maxp_fixture_pending)
    otvalid_expectation_mismatch_pending = otvalid_expectation_mismatch_pending_reason(row)
    if otvalid_expectation_mismatch_pending:
        return ("pending-route", otvalid_expectation_mismatch_pending)
    residual_public_pending = residual_public_surface_pending_reason(row)
    if residual_public_pending:
        return ("pending-route", residual_public_pending)
    if row.expect_error and not row.compare_error_output:
        return (
            "generic-error-fallback",
            "expected error is accepted without exact C status/output comparison",
        )
    if row.allow_oracle_errors:
        return (
            "generic-fallback",
            "oracle errors are explicitly accepted instead of requiring the declared route",
        )
    if row.case_id in SHARED_RUNTIME_FALLBACK_CASES:
        return (
            "generic-fallback",
            "shared Rust fallback across Rust, C ABI, and WASM actual lanes; not public-route parity",
        )
    size_null_reason = size_null_validation_reason(row)
    if size_null_reason:
        return ("real-null-validation", size_null_reason)
    otvalid_null_reason = otvalid_null_validation_reason(row)
    if otvalid_null_reason:
        return ("real-null-validation", otvalid_null_reason)
    otvalid_real_reason = otvalid_real_parity_reason(row)
    if otvalid_real_reason:
        return ("real-parity", otvalid_real_reason)
    font_format_real_reason = font_format_real_parity_reason(row)
    if font_format_real_reason:
        return ("real-parity", font_format_real_reason)
    outline_get_bitmap_real_reason = outline_get_bitmap_real_parity_reason(row)
    if outline_get_bitmap_real_reason:
        return ("real-parity", outline_get_bitmap_real_reason)
    set_debug_hook_real_reason = set_debug_hook_real_parity_reason(row)
    if set_debug_hook_real_reason:
        return ("real-parity", set_debug_hook_real_reason)
    add_default_modules_real_reason = add_default_modules_real_parity_reason(row)
    if add_default_modules_real_reason:
        return ("real-parity", add_default_modules_real_reason)
    focused_success_real_reason = focused_success_real_parity_reason(row)
    if focused_success_real_reason:
        return ("real-parity", focused_success_real_reason)
    done_mm_var_real_reason = done_mm_var_real_parity_reason(row)
    if done_mm_var_real_reason:
        return ("real-parity", done_mm_var_real_reason)
    wrapper_null_reason = wrapper_null_validation_reason(row)
    if wrapper_null_reason:
        return ("real-null-validation", wrapper_null_reason)
    raw_slot_null_reason = raw_slot_null_validation_reason(row)
    if raw_slot_null_reason:
        return ("real-parity", raw_slot_null_reason)
    if operation_is_real_parity(row.operation):
        return ("real-parity", "explicit C oracle, Rust FFI, C ABI, and WASM route")
    if row.expect_error and not has_runtime_asset(row):
        return ("generic-error-fallback", "no-asset expected-error row")
    if not row.expect_error and not has_runtime_asset(row) and has_null_lifecycle_handle(row):
        return ("void-fallback", "no-asset null-handle void row")
    return ("generic-fallback", "no explicit maintained route classification")


def supplementary_safe_api_flags(row: ConcreteInput) -> list[str]:
    return [
        flag
        for flag in sorted(SUPPLEMENTARY_SAFE_API_FLAGS)
        if row.params.get(flag) is True
    ]


def runtime_id(row: ConcreteInput) -> str:
    if row.variant_id:
        return f"{row.case_id}@{row.variant_id}"
    return row.case_id


def build_route_audit(items: dict[str, ManifestSubject]) -> dict[str, object]:
    rows = []
    for row in concrete_inputs(items):
        category, reason = route_category(row)
        supplementary_flags = supplementary_safe_api_flags(row)
        rows.append(
            {
                "subject": row.subject,
                "case_id": row.case_id,
                "runtime_id": runtime_id(row),
                "case": row.case,
                "operation": row.operation,
                "category": category,
                "reason": reason,
                "expect_error": row.expect_error,
                "compare_error_output": row.compare_error_output,
                "allow_oracle_errors": row.allow_oracle_errors,
                "expectation_status": row.expectation_status,
                "supplementary_safe_api_flags": supplementary_flags,
            }
        )

    category_counts: dict[str, int] = {}
    operation_counts: dict[str, dict[str, int]] = {}
    examples: dict[tuple[str, str], str] = {}
    pending_core_rows: list[str] = []
    pending_route_rows: list[str] = []
    runtime_pending_rows: list[str] = []
    placeholder_style_rows: list[str] = []
    supplementary_counts: dict[str, int] = {}
    supplementary_examples: dict[str, str] = {}
    for row in rows:
        category_counts[row["category"]] = category_counts.get(row["category"], 0) + 1
        by_category = operation_counts.setdefault(row["operation"], {})
        by_category[row["category"]] = by_category.get(row["category"], 0) + 1
        examples.setdefault((row["operation"], row["category"]), row["runtime_id"])
        if row["category"] == "pending-core":
            pending_core_rows.append(row["runtime_id"])
            if row["runtime_id"] not in AUDIT_ONLY_PENDING_CORE_CASES:
                runtime_pending_rows.append(row["runtime_id"])
        if row["category"] == "pending-route":
            pending_route_rows.append(row["runtime_id"])
            runtime_pending_rows.append(row["runtime_id"])
        if row["category"] in PLACEHOLDER_STYLE_CATEGORIES:
            placeholder_style_rows.append(row["runtime_id"])
        flags = row["supplementary_safe_api_flags"]
        assert isinstance(flags, list)
        for flag in flags:
            label = SUPPLEMENTARY_SAFE_API_FLAGS[flag]
            supplementary_counts[label] = supplementary_counts.get(label, 0) + 1
            supplementary_examples.setdefault(label, row["runtime_id"])

    return {
        "total_concrete_cases": len(rows),
        "category_counts": dict(sorted(category_counts.items())),
        "goal_ledger": {
            "runtime_pending": len(runtime_pending_rows),
            "route_pending": len(pending_route_rows),
            "route_core_pending": len(pending_core_rows),
            "route_or_core_pending": len(pending_route_rows) + len(pending_core_rows),
            "green_placeholder_style_rows": len(placeholder_style_rows),
        },
        "runtime_pending_rows": sorted(runtime_pending_rows),
        "route_pending_rows": sorted(pending_route_rows),
        "route_core_pending_rows": sorted(pending_core_rows),
        "placeholder_style_category_counts": {
            category: category_counts.get(category, 0)
            for category in sorted(PLACEHOLDER_STYLE_CATEGORIES)
            if category_counts.get(category, 0)
        },
        "supplementary_safe_api_assertions": {
            "total_assertions": sum(supplementary_counts.values()),
            "counts": dict(sorted(supplementary_counts.items())),
            "examples": dict(sorted(supplementary_examples.items())),
            "note": (
                "These assertions prove high-level Rust convenience behavior on "
                "existing public FreeType rows, but they are not independent C "
                "FreeType public API routes."
            ),
        },
        "operation_counts": dict(sorted(operation_counts.items())),
        "examples": {
            f"{operation}|{category}": example
            for (operation, category), example in sorted(examples.items())
        },
        "rows": rows,
    }


def write_route_audit(report: dict[str, object], json_path: Path, md_path: Path) -> None:
    json_path.parent.mkdir(parents=True, exist_ok=True)
    md_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")

    operation_counts = report["operation_counts"]
    assert isinstance(operation_counts, dict)
    examples = report["examples"]
    assert isinstance(examples, dict)

    lines = [
        "# Unified Public API Route Audit",
        "",
        f"Concrete cases: {report['total_concrete_cases']}",
        "",
        "## Category Counts",
        "",
        "| Category | Cases |",
        "|---|---:|",
    ]
    category_counts = report["category_counts"]
    assert isinstance(category_counts, dict)
    for category, count in category_counts.items():
        lines.append(f"| {category} | {count} |")

    goal_ledger = report["goal_ledger"]
    assert isinstance(goal_ledger, dict)
    lines.extend(
        [
            "",
            "## Goal Ledger",
            "",
            "| Ledger | Cases |",
            "|---|---:|",
            f"| runtime pending | {goal_ledger['runtime_pending']} |",
            f"| route pending | {goal_ledger['route_pending']} |",
            f"| full route/core pending | {goal_ledger['route_core_pending']} |",
            f"| route or core pending | {goal_ledger['route_or_core_pending']} |",
            f"| green placeholder-style rows | {goal_ledger['green_placeholder_style_rows']} |",
            "",
            "Runtime pending combines `pending-route` rows with the subset of `pending-core` rows that the unified runtime parity suite reports as pending. "
            "The route-or-core ledger also includes audit-visible core rows that are not current runnable runtime parity cases.",
        ]
    )

    route_pending_rows = report["route_pending_rows"]
    assert isinstance(route_pending_rows, list)
    if route_pending_rows:
        lines.extend(["", "### Route-Pending Rows", ""])
        for runtime_id in route_pending_rows:
            lines.append(f"- `{runtime_id}`")

    pending_rows = report["route_core_pending_rows"]
    assert isinstance(pending_rows, list)
    if pending_rows:
        lines.extend(["", "### Pending Rows", ""])
        for runtime_id in pending_rows:
            lines.append(f"- `{runtime_id}`")

    placeholder_counts = report["placeholder_style_category_counts"]
    assert isinstance(placeholder_counts, dict)
    if placeholder_counts:
        lines.extend(
            [
                "",
                "### Placeholder-Style Categories",
                "",
                "| Category | Cases |",
                "|---|---:|",
            ]
        )
        for category, count in placeholder_counts.items():
            lines.append(f"| {category} | {count} |")

    supplementary = report["supplementary_safe_api_assertions"]
    assert isinstance(supplementary, dict)
    supplementary_counts = supplementary["counts"]
    assert isinstance(supplementary_counts, dict)
    if supplementary_counts:
        lines.extend(
            [
                "",
                "## Supplementary Non-FreeType Goal Assertions",
                "",
                str(supplementary["note"]),
                "",
                "| Surface | Rows | Example |",
                "|---|---:|---|",
            ]
        )
        supplementary_examples = supplementary["examples"]
        assert isinstance(supplementary_examples, dict)
        for surface, count in supplementary_counts.items():
            example = supplementary_examples.get(surface, "")
            lines.append(f"| {surface} | {count} | `{example}` |")
        lines.extend(
            [
                "",
                "The following surfaces should not drive the public FreeType manifest or coverage goal:",
            ]
        )
        for surface in NON_FREETYPE_GOAL_SURFACES:
            lines.append(f"- {surface}")

    lines.extend(
        [
            "",
            "## Operations Needing R0 Disposition",
            "",
            "| Operation | Category | Cases | Example |",
            "|---|---|---:|---|",
        ]
    )
    flagged = {
        "generic-fallback",
        "generic-error-fallback",
        "null-error-fallback",
        "shape-incomplete-fallback",
        "void-fallback",
        "explicit-unsupported",
        "pending-route",
        "pending-core",
    }
    for operation, counts in operation_counts.items():
        assert isinstance(counts, dict)
        for category, count in sorted(counts.items()):
            if category not in flagged:
                continue
            example = examples.get(f"{operation}|{category}", "")
            lines.append(f"| `{operation}` | {category} | {count} | `{example}` |")
    md_path.write_text("\n".join(lines) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("subjects", nargs="*")
    parser.add_argument("--audit-json", type=Path, default=DEFAULT_AUDIT_JSON)
    parser.add_argument("--route-audit", action="store_true")
    parser.add_argument("--route-audit-json", type=Path, default=DEFAULT_ROUTE_AUDIT_JSON)
    parser.add_argument("--route-audit-md", type=Path, default=DEFAULT_ROUTE_AUDIT_MD)
    args = parser.parse_args()

    subjects = read_manifest()
    selected = args.subjects
    if selected:
        items = {subject: subjects[subject] for subject in selected}
    else:
        items = subjects

    if args.route_audit:
        report = build_route_audit(items)
        write_route_audit(report, args.route_audit_json, args.route_audit_md)
        print(f"wrote route audit json {args.route_audit_json}")
        print(f"wrote route audit markdown {args.route_audit_md}")
        print(
            "route audit concrete_cases={} category_counts={}".format(
                report["total_concrete_cases"],
                report["category_counts"],
            )
        )
        return 0

    errors: list[str] = []
    if not selected:
        errors.extend(check_manifest_public_surface(subjects, args.audit_json))
        errors.extend(check_input_directory(subjects))
        errors.extend(check_ffi_abi_surface(args.audit_json))
    for subject in items.values():
        errors.extend(check_file(subject))

    if errors:
        for error in errors:
            print(error)
        return 1
    print(f"checked {len(items)} public API input files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
