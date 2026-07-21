# Bzip2 disabled-build parity note

The pinned FreeType oracle build used by `pillow-rs-freetype` passes
`-DFT_DISABLE_BZIP2=ON` in `scripts/build_ft.sh`.

For that build, FreeType 2.14.3 `src/bzip2/ftbzip2.c:521-529` returns
`FT_Err_Unimplemented_Feature` from `FT_Stream_OpenBzip2` before it validates
null stream/source handles or inspects the source bytes.

That means rows describing enabled-build behavior, such as invalid input
validation or invalid/truncated Bzip2 headers, are not real parity rows for the
active pinned oracle build. They should stay pending or be split by build
configuration until a Bzip2-enabled oracle variant is added.

The maintained active-build route is:

- `ftbzip2.FT_Stream_OpenBzip2.out_of_scope_uncompiled_bzip2_policy`

That row validates the disabled-build policy through the pinned C oracle, Rust
FFI, C ABI, and WASM ABI. It must not be used to claim enabled-Bzip2 stream
parity.
