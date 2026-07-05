# Notice

This crate is a Rust migration of FreeType-compatible font loading, hinting,
metrics, outline, and rasterization behavior.

The project is distributed under the FreeType License (`FTL`). The unmodified
license text is retained in `FTL.TXT` and duplicated in `LICENSE` for package
tooling that expects a root license file.

The vendored C FreeType source under `freetype/` is retained as a version-pinned
oracle for fixture generation, diagnosis, and line-by-line behavior comparison.
It is not linked into runtime Rust code.

The Rust implementation under `src/`, tests under `tests/`, and maintained
tooling under `scripts/` are additions and migrations written for this crate.
They are intended to reproduce FreeType behavior from Rust code while preserving
the FreeType attribution and license requirements.

Recommended attribution for downstream documentation:

```text
Portions of this software are copyright © 1996-2026 The FreeType
Project (https://freetype.org). All rights reserved.
```
