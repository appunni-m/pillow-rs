# COLRv1 Paint Parity Batch Plan

## Goal

Move the `ftcolor.*` COLRv1 pending family toward exact same-input parity
across pinned C FreeType, pure Rust FFI, thin C ABI, and WASM ABI without
counting unsupported rows as green placeholders.

The current route audit groups this as the largest shared pending family:
96 pending rows across paint graph traversal, paint payload reads, color lines,
gradients, transforms, clip boxes, palette foreground behavior, and layer
iteration.

## Batch 1: solid, glyph, and composite paint graph

Scope:

- Generate a maintained compact COLRv1 fixture at the existing
  manifest-standard path:
  `tests/fixtures/fonts/color/colr_v1_composite_modes.ttf`.
- Cover root `PaintSolid`.
- Cover nested `PaintGlyph`.
- Cover `PaintComposite` for every real `FT_Composite_Mode` value.
- Add pure-Rust COLRv1 parsing for only the covered paint formats.
- Add same-input oracle, Rust FFI, C ABI, and WASM routes that compare:
  - `FT_Get_Color_Glyph_Paint` boolean return and root opaque paint fields;
  - `FT_Get_Paint` format dispatch;
  - `FT_PaintSolid` palette index and alpha;
  - `FT_PaintGlyph` child paint handle and glyph ID;
  - `FT_PaintComposite` source/backdrop handles and composite mode;
  - traversal order for source and backdrop child paints.

Out of scope for this batch:

- Gradients and `FT_ColorLine`.
- `FT_Get_Colorline_Stops`.
- Transforms and root transform insertion.
- Clip boxes.
- `FT_Get_Paint_Layers`.
- Variable COLRv1 payloads.

Those rows must remain pending until exact routes exist. Do not reclassify them
as real parity based on this fixture alone.

## Batch 2: exact payload rows for covered paint formats

Scope:

- Reuse `tests/fixtures/fonts/color/colr_v1_composite_modes.ttf`.
- Promote only rows whose declared behavior is fully present in that fixture:
  - `FT_COLR_PAINTFORMAT_SOLID`;
  - `FT_COLR_PAINTFORMAT_GLYPH`;
  - `FT_COLR_PAINTFORMAT_COMPOSITE`;
  - `FT_PaintSolid`;
  - `FT_PaintGlyph`;
  - `FT_PaintComposite`;
  - `FT_OpaquePaint` produced/consumed by root and nested paint APIs;
  - `FT_Get_Color_Glyph_Paint` downstream graph contract for the covered
    solid/glyph/composite graph.
- Keep broad all-paint-format, gradients, transforms, colorlines, layer lists,
  COLR glyph, foreground-color, and malformed-paint rows pending.

Acceptance gates:

```bash
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint_graph_node
make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_paint_and_get_paint
make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_paint_and_resolve
make -C pillow-rs-freetype test-op OP=ftcolor.traverse_color_paint_graph
make fontdone-ffi-compat
make fontdone-lint
make fontdone-parity
```

## Acceptance gates

Focused:

```bash
make -C pillow-rs-freetype font-fixture-color
make -C pillow-rs-freetype test-op OP=ftcolor.get_color_glyph_paint
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint
make -C pillow-rs-freetype test-op OP=ftcolor.get_paint_graph
make -C pillow-rs-freetype test-op OP=ftcolor.traverse_paint_graph
```

Broad:

```bash
make fontdone-ffi-compat
make fontdone-lint
make fontdone-parity
```

## Non-placeholder rule

A row can move from pending only after the declared fixture path exists and the
same concrete input is executed against pinned C FreeType, Rust FFI, thin C ABI,
and WASM ABI. Constants, layout checks, or a local parser smoke test are not
runtime parity.
