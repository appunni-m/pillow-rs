# Library Lifecycle API/ABI Slice

Scope: FreeType 2.14.3 library creation/destruction, library version,
library reference counting, custom allocator bootstrap, default module
registration, module lookup/removal, driver properties, renderer selection, and
closely related module/version probes.

Primary C references:

- `FT_FREETYPE_H` / `freetype/freetype.h`
- `FT_MODULE_H` / `freetype/ftmodapi.h`
- `FT_RENDER_H` / `freetype/ftrender.h`
- `FT_SYSTEM_H` / `freetype/ftsystem.h`
- https://freetype.org/freetype2/docs/reference/ft2-library_setup.html
- https://freetype.org/freetype2/docs/reference/ft2-module_management.html
- https://freetype.org/freetype2/docs/reference/ft2-system_interface.html

Servo `rust-freetype` is useful as a raw C binding checklist. It is not the
target behavior. The target is the pinned C FreeType 2.14.3 headers and oracle
behavior.

## Current fontdone State

`src/api.rs` currently exposes a semantic safe Rust facade:

- `Library::init() -> Library`
- `Library::new_memory_face(data, face_index, size_pt) -> Result<Face, FontError>`
- `drop Library`
- `Face::from_memory(...)`

`Library` is currently `#[derive(Debug, Clone, Copy, Default)] pub struct
Library;`. It is zero-sized and has no owned allocator, module registry,
child-object registry, reference count, version method, property store, or
renderer registry. This is enough for common Rust construction flow, but it is
not C ABI lifecycle compatibility.

`tests/data/interface_map.json` currently classifies this slice as:

| C symbol | Current fontdone mapping | Current status | Slice status |
|---|---|---|---|
| `FT_Init_FreeType` | `Library::init` | `complete` | semantic only; ABI missing |
| `FT_Done_FreeType` | `drop Library` | `complete` | semantic only; ABI missing |
| `FT_Library_Version` | none | `planned` | missing |
| `FT_Reference_Library` | none | `out_of_scope` | should become planned for ABI |
| `FT_New_Library` | none | `out_of_scope` | should become planned for ABI |
| `FT_Done_Library` | none | `out_of_scope` | should become planned for ABI |
| `FT_Add_Default_Modules` | none | `out_of_scope` | required for `FT_New_Library` parity |
| `FT_Add_Module` | none | `out_of_scope` | decide real support vs exact unsupported ABI |
| `FT_Get_Module` | none | `out_of_scope` | required for built-in module visibility |
| `FT_Remove_Module` | none | `out_of_scope` | decide real support vs exact unsupported ABI |
| `FT_Property_Set` | none | `out_of_scope` | required for driver-property ABI |
| `FT_Property_Get` | none | `out_of_scope` | required for driver-property ABI |
| `FT_Set_Default_Properties` | none | `out_of_scope` | required if env properties are supported |
| `FT_Get_Renderer` | none | `out_of_scope` | required for renderer ABI |
| `FT_Set_Renderer` | none | `out_of_scope` | required for renderer ABI |
| `FT_Set_Debug_Hook` | none | `out_of_scope` | ABI symbol can be unsupported/no-op |
| `FT_Get_TrueType_Engine_Type` | none | `planned` | belongs with version/property probes |

The existing `out_of_scope` reason says the pure Rust crate has no dynamic
module registry. That remains true for the safe facade. A future C ABI
replacement still needs linkable symbols and C-compatible behavior. It may
return exact unsupported errors for external module extension, but it must
model built-in modules/properties that affect glyph behavior.

## C Symbols and Required ABI Shape

### Basic Library Setup

| C symbol | C signature | Required behavior |
|---|---|---|
| `FT_Init_FreeType` | `FT_Error FT_Init_FreeType(FT_Library *alibrary)` | Allocate one independent library, install build-time default modules, apply default properties when configured, set `*alibrary`, return `0` on success. |
| `FT_Done_FreeType` | `FT_Error FT_Done_FreeType(FT_Library library)` | Destroy a library and all child faces/sizes/glyph objects. Should behave like C's normal teardown path for a library created by `FT_Init_FreeType`. |
| `FT_Library_Version` | `void FT_Library_Version(FT_Library library, FT_Int *amajor, FT_Int *aminor, FT_Int *apatch)` | Write FreeType source version `2, 14, 3` to non-null output pointers. Must match C behavior for null output pointers and null/invalid `library`. |
| `FREETYPE_MAJOR` | macro value `2` | Future generated headers must expose exact numeric macro. |
| `FREETYPE_MINOR` | macro value `14` | Future generated headers must expose exact numeric macro. |
| `FREETYPE_PATCH` | macro value `3` | Future generated headers must expose exact numeric macro. |

### Custom Allocator and Reference-Counted Setup

| C symbol/type | C signature/shape | Required behavior |
|---|---|---|
| `FT_New_Library` | `FT_Error FT_New_Library(FT_Memory memory, FT_Library *alibrary)` | Create a library using caller-supplied memory callbacks. The `FT_MemoryRec` must remain valid until library teardown. Does not by itself imply default modules are registered. |
| `FT_Done_Library` | `FT_Error FT_Done_Library(FT_Library library)` | Decrement reference count when greater than one; otherwise destroy modules, children, and library storage. |
| `FT_Reference_Library` | `FT_Error FT_Reference_Library(FT_Library library)` | Increment library reference count that starts at `1` after creation. |
| `FT_Memory` | `typedef struct FT_MemoryRec_* FT_Memory` | Opaque pointer to memory callbacks. |
| `FT_MemoryRec` | fields `user`, `alloc`, `free`, `realloc` | Future ABI type must be `#[repr(C)]` with exact field order and pointer/function-pointer widths. |
| `FT_Alloc_Func` | `void* (*)(FT_Memory memory, long size)` | Called for C ABI allocations when custom memory is active. |
| `FT_Free_Func` | `void (*)(FT_Memory memory, void* block)` | Called for C ABI frees. |
| `FT_Realloc_Func` | `void* (*)(FT_Memory memory, long cur_size, long new_size, void* block)` | On allocation failure the old block must remain valid. |

The future ABI crate may export `extern "C"` functions because exporting the C
ABI is its purpose. It still must be backed by Rust implementation code only and
must not call native FreeType at runtime.

### Module Lifecycle

| C symbol/type | C signature/shape | Required behavior |
|---|---|---|
| `FT_Module` | `typedef struct FT_ModuleRec_* FT_Module` | Opaque handle to driver/renderer/module instance. |
| `FT_Module_Class` | fields `module_flags`, `module_size`, `module_name`, `module_version`, `module_requires`, `module_interface`, `module_init`, `module_done`, `get_interface` | Future ABI type must be `#[repr(C)]`; external module callbacks require a C ABI facade design. |
| `FT_Add_Default_Modules` | `void FT_Add_Default_Modules(FT_Library library)` | Register the same built-in modules as pinned C FreeType where in scope. Needed after `FT_New_Library`. |
| `FT_Add_Module` | `FT_Error FT_Add_Module(FT_Library library, const FT_Module_Class *clazz)` | Add a module; C returns errors for duplicate name or too-new `module_requires`. If external modules are not supported, expose the symbol and return the exact measured C-compatible error bucket. |
| `FT_Get_Module` | `FT_Module FT_Get_Module(FT_Library library, const char *module_name)` | Return built-in module handles by ASCII name or null if missing. Important names include `truetype`, `sfnt`, `smooth`, `raster1`, and `autofitter` if the Rust engine claims equivalent built-ins. |
| `FT_Remove_Module` | `FT_Error FT_Remove_Module(FT_Library library, FT_Module module)` | Remove module and destroy it on success. If removing built-ins is unsupported, return the exact measured C-compatible error. |
| `FT_Set_Debug_Hook` | `void FT_Set_Debug_Hook(FT_Library library, FT_UInt hook_index, FT_DebugHook_Func debug_hook)` | Export for link parity. The TrueType interpreter hook can be a no-op unless debugging parity later requires it. |

The safe Rust API does not need public dynamic module management. For the C ABI,
there should be an internal `LibraryState` with stable handles for built-ins and
measured behavior for unsupported dynamic modules.

### Properties, Renderers, and Version-Like Probes

| C symbol/type | C signature/shape | Required behavior |
|---|---|---|
| `FT_Property_Set` | `FT_Error FT_Property_Set(FT_Library library, const FT_String *module_name, const FT_String *property_name, const void *value)` | Set known driver properties with exact type validation and error codes. |
| `FT_Property_Get` | `FT_Error FT_Property_Get(FT_Library library, const FT_String *module_name, const FT_String *property_name, void *value)` | Get known driver properties with exact type validation and error codes. |
| `FT_Set_Default_Properties` | `void FT_Set_Default_Properties(FT_Library library)` | If environment-property support is chosen, parse `FREETYPE_PROPERTIES` like C; otherwise no-op like C when the option is disabled. The chosen build policy must be explicit in generated headers/tests. |
| `FT_Get_Renderer` | `FT_Renderer FT_Get_Renderer(FT_Library library, FT_Glyph_Format format)` | Return current renderer for outline glyphs or null. |
| `FT_Set_Renderer` | `FT_Error FT_Set_Renderer(FT_Library library, FT_Renderer renderer, FT_UInt num_params, FT_Parameter *parameters)` | Set renderer for its glyph format. C docs say current renderers do not use `parameters`; tests should pass null and nonzero cases to measure errors. |
| `FT_Renderer` | `typedef struct FT_RendererRec_* FT_Renderer` | Opaque module handle. |
| `FT_Renderer_Class` | `root`, `glyph_format`, `render_glyph`, `transform_glyph`, `get_glyph_cbox`, `set_mode`, `raster_class` | Needed only if dynamic renderer class ABI is implemented. |
| `FT_Get_TrueType_Engine_Type` | `FT_TrueTypeEngineType FT_Get_TrueType_Engine_Type(FT_Library library)` | Return exact FreeType 2.14.3 value for the chosen interpreter behavior. Must be measured from the pinned oracle build and tied to native TrueType bytecode support. |

Driver properties that can affect current fontdone output and should be
prioritized:

- `truetype:interpreter-version`
- `autofitter:*` properties that change hinting
- LCD/subpixel renderer properties if exposed by the pinned build

Properties for unsupported drivers (`cff`, `type1`, `pcf`, etc.) still need
link/error compatibility decisions, but they should not be used to claim output
parity until those drivers exist.

## Representation Plan

Add a separate ABI layer rather than changing the safe `Library` facade into a
raw pointer API.

Suggested internal split:

- `Library`: keep the existing safe Rust facade for Rust callers.
- `abi::FT_LibraryRec`: `#[repr(C)]` opaque-owned state for exported symbols.
- `abi::FT_MemoryRec`: `#[repr(C)]` exact public memory callback record.
- `abi::FT_ModuleRec`, `abi::FT_RendererRec`: opaque handles to internal module
  descriptors.
- `LibraryState`: Rust-owned implementation state containing refcount, memory
  policy, built-in module table, property store, renderer mapping, and child
  object registry.

Required state:

- Reference count initialized to `1`.
- Version constants `(2, 14, 3)`.
- Default module registry with deterministic insertion order.
- Property map for supported built-in modules.
- Renderer mapping keyed by `FT_Glyph_Format`.
- Child face/size handles owned or invalidated on library teardown.
- Memory policy:
  - system allocator for `FT_Init_FreeType`;
  - caller callbacks for `FT_New_Library`;
  - no custom allocator callback use from core Rust collections unless the ABI
    layer owns that allocation boundary explicitly.

`#[repr(C)]` needs:

- `FT_MemoryRec`
- `FT_Module_Class`
- `FT_Renderer_Class`
- `FT_Parameter`
- public scalar aliases and constants used in signatures, including `FT_Error`,
  `FT_Glyph_Format`, `FT_UInt`, `FT_Int`, `FT_Long`, `FT_ULong`, `FT_Fixed`,
  and `FT_String`

Do not expose Rust-only structs such as the current zero-sized `Library`,
`Face`, or `FontError` as proof of ABI compatibility. They can back the ABI
implementation but are not layout-compatible records.

## Error and Lifetime Semantics To Match

The following must be measured against the pinned C oracle before coding final
assertions:

- `FT_Init_FreeType(NULL)` exact error and whether any memory is touched.
- `FT_Done_FreeType(NULL)` exact error.
- `FT_New_Library(NULL, &library)` and `FT_New_Library(memory, NULL)` exact
  errors and whether output is cleared.
- `FT_Done_Library(NULL)` exact error.
- `FT_Reference_Library(NULL)` exact error.
- `FT_Add_Default_Modules(NULL)` behavior.
- `FT_Get_Module(NULL, "truetype")`, `FT_Get_Module(library, NULL)`, and
  unknown module name behavior.
- `FT_Property_Set/Get` invalid library, missing module, missing property,
  wrong value pointer, and known property success behavior.
- `FT_Get_Renderer` null library and unsupported format behavior.
- `FT_Set_Renderer` null library, null renderer, foreign renderer, and
  parameter handling behavior.

Lifetime rules to implement:

- Every `FT_Library` is independent.
- `FT_Done_FreeType`/`FT_Done_Library` destroys child faces, sizes, glyph slots,
  modules, renderers, streams, and memory owned by the library.
- `FT_Done_Library` with refcount greater than one decrements only.
- `FT_Reference_Library` keeps the handle valid until matching
  `FT_Done_Library` calls release it.
- `FT_New_Library` must not take ownership of the caller's `FT_MemoryRec`; C
  requires it to remain valid for the life of the library.
- After successful final teardown, stale C handles are invalid to use; tests
  should avoid relying on undefined use-after-free behavior except under
  sanitizer-only diagnostics.

## Shared Dynamic Test Strategy

Add a maintained C oracle probe under `pillow-rs-freetype/scripts/` and drive it
through `make -C pillow-rs-freetype api-abi-audit` or a new Make target if the
probe becomes a gate. The probe should compile once against pinned C FreeType
2.14.3 and later against the Rust ABI library/header. It should print stable
JSON, not free-form text.

Minimum scenarios:

1. Version:
   - call `FT_Init_FreeType`;
   - call `FT_Library_Version` with all outputs;
   - call again with each output pointer null;
   - assert `2.14.3` and exact null-output behavior.
2. Basic lifecycle:
   - create two libraries;
   - load the same memory face through each;
   - destroy one and verify the other still works;
   - destroy both.
3. Null/error matrix:
   - run every invalid argument case listed above;
   - record exact `FT_Error` numeric value and symbolic name from headers.
4. Reference count:
   - create with `FT_New_Library`;
   - `FT_Add_Default_Modules`;
   - `FT_Reference_Library`;
   - call `FT_Done_Library` once and verify a face load still works;
   - call it again and stop using the handle.
5. Custom allocator:
   - provide counting `alloc/free/realloc` callbacks;
   - verify `FT_New_Library` and `FT_Done_Library` balance C-visible
     allocations for the ABI layer;
   - verify failed `realloc` leaves the old block valid.
6. Default modules:
   - after `FT_Init_FreeType`, query common built-ins with `FT_Get_Module`;
   - after `FT_New_Library` before defaults, query the same names;
   - after `FT_Add_Default_Modules`, query again;
   - record exact present/missing set for the pinned build.
7. Module add/remove:
   - duplicate default module addition behavior;
   - invalid class pointer behavior;
   - too-new `module_requires` behavior;
   - remove known module if the C probe can do it without making later tests
     order-dependent.
8. Properties:
   - get/set `truetype:interpreter-version`;
   - invalid module/property/value cases;
   - `FT_Set_Default_Properties` with and without `FREETYPE_PROPERTIES`.
9. Renderers:
   - get renderer for `FT_GLYPH_FORMAT_OUTLINE`;
   - set the same renderer with `num_params = 0, parameters = NULL`;
   - invalid renderer/format cases.
10. TrueType engine:
    - call `FT_Get_TrueType_Engine_Type` after normal init and custom init;
    - record exact enum value.

The generated JSON should be checked in only if it is an oracle fixture with a
documented regeneration path. Do not hand-edit generated outputs.

## Implementation Order

1. Add exact numeric constants and `#[repr(C)]` ABI type definitions for this
   slice.
2. Add non-exported Rust `LibraryState` and safe internal constructors that can
   back both system and custom memory setup.
3. Implement `FT_Library_Version` first; it has no module dependency and proves
   the header/export path.
4. Implement `FT_Init_FreeType`/`FT_Done_FreeType` with system allocation and
   child cleanup.
5. Implement `FT_New_Library`/`FT_Done_Library`/`FT_Reference_Library` and
   allocator callback accounting.
6. Add default built-in module descriptors and `FT_Add_Default_Modules`.
7. Add lookup-only `FT_Get_Module`, then decide whether `FT_Add_Module` and
   `FT_Remove_Module` are real dynamic extension support or exact unsupported
   ABI shims.
8. Add property storage for behavior-affecting built-ins.
9. Add renderer get/set handles for outline rendering.
10. Promote each symbol from planned/out-of-scope to implemented only after C
    compile/link and behavior probes pass.

## Remaining Risks

- Exact FreeType error values for invalid lifecycle/module/property inputs have
  not been measured by a behavior probe yet. `make -C pillow-rs-freetype
  api-abi-audit` confirms headers/signatures, but it does not execute the
  invalid-input matrix described above.
- The current safe `Library` is `Copy`, which is harmless for a zero-sized Rust
  facade but not compatible with C refcounted ownership. The ABI layer must not
  expose this type directly.
- Supporting external `FT_Add_Module` callbacks would require accepting C
  function pointers in the ABI layer. That is ABI export work, not runtime FFI
  to native FreeType, but it still needs a clear safety boundary.
- Environment-driven properties can make output process-global unless scoped
  carefully to the created library. Tests must isolate `FREETYPE_PROPERTIES`.
- Module and property compatibility can affect glyph parity. Do not mark these
  symbols complete solely because they link; verify behavior-affecting property
  changes against rendered/metric lanes.
