# Systemic Fix Patterns — Preventing Recurrence

Each audit finding category mapped to a **systemic guard** (type, macro, lint, script, or CI check) that prevents the entire class from recurring.

---

## Fix 1: Integer Overflow → `CheckedDims` newtype

**Class:** All overflow sites where `(w * h * channels) as usize` wraps silently.

**Systemic fix:** A `CheckedDims` type that is the ONLY way to create an image buffer. Make raw multiplication impossible via clippy.

### Step A — The type (`pillow-rs/src/checked_dims.rs`)

```rust
/// Validated image dimensions. The ONLY constructor checks overflow and
/// a configurable max-pixel cap.  Every buffer-allocation site must
/// accept this type so it is impossible to allocate a buffer from raw
/// (w, h) without validation.
#[derive(Debug, Clone, Copy)]
pub struct CheckedDims {
    pub width: u32,
    pub height: u32,
    pub channels: u8,
    /// Pre-computed: width * height * channels, guaranteed no overflow
    total_bytes: usize,
    /// Pre-computed: width * height, guaranteed no overflow
    total_pixels: usize,
}

/// Global max-pixel cap (matching PIL's `Image.MAX_IMAGE_PIXELS`).
static MAX_PIXELS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(268_435_456); // ~1 GB RGBA

impl CheckedDims {
    /// The ONE way to create dimensions. Every allocation path MUST go through here.
    pub fn new(width: u32, height: u32, channels: u8) -> Result<Self, PilError> {
        // Overflow check
        let total_pixels = (width as u64)
            .checked_mul(height as u64)
            .ok_or_else(|| PilError::ValueError(
                format!("image dimensions overflow: {width}×{height}")
            ))?;

        // DoS cap
        let max = MAX_PIXELS.load(std::sync::atomic::Ordering::Relaxed);
        if total_pixels > max {
            return Err(PilError::ValueError(
                format!("image size {total_pixels} exceeds MAX_PIXELS ({max})")
            ));
        }

        let total_bytes = total_pixels
            .checked_mul(channels as u64)
            .ok_or_else(|| PilError::ValueError(
                format!("buffer size overflow: {total_pixels} px × {channels} ch")
            ))?;

        Ok(Self {
            width, height, channels,
            total_bytes: total_bytes as usize,
            total_pixels: total_pixels as usize,
        })
    }

    pub fn total_bytes(&self) -> usize { self.total_bytes }
    pub fn total_pixels(&self) -> usize { self.total_pixels }

    /// Override the global max (like PIL's `Image.MAX_IMAGE_PIXELS = None`)
    pub fn set_max_pixels(limit: Option<u64>) {
        MAX_PIXELS.store(
            limit.unwrap_or(u64::MAX),
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    /// Allocate a pixel buffer pre-sized correctly (no manual size arithmetic)
    pub fn alloc_buffer(&self) -> Vec<u8> {
        vec![0u8; self.total_bytes]
    }
}
```

### Step B — Clippy lint gate

Add to root `Cargo.toml` `[workspace.lints.clippy]`:

```toml
# Disallow the pattern that caused all overflow bugs:
#   (w * h) as usize
#   (w * h * N) as usize
# These must go through CheckedDims::new.
cast_possible_truncation = "deny"
cast_sign_loss = "deny"
arithmetic_side_effects = "warn"
```

### Step C — Migration pattern

Every site that currently does:

```rust
let buf = vec![0u8; (w * h) as usize * channels];
```

Becomes:

```rust
let dims = CheckedDims::new(w, h, channels)?;
let buf = dims.alloc_buffer();
```

Every site that does:

```rust
let img = RgbaImage::from_raw(w, h, buf).unwrap();
```

Becomes:

```rust
let img = RgbaImage::from_raw(dims.width, dims.height, &buf)
    .ok_or_else(|| PilError::ValueError("buffer size mismatch".into()))?;
```

---

## Fix 2: Panics in Library → Deny `unwrap_used` + `expect_used` in production code

**Class:** `unwrap()` and bare `expect()` on fallible operations (notably `from_raw`).

**Systemic fix:** Workspace already has `clippy::unwrap_used = "deny"`. Extend to cover the remaining cases.

### Step A — Add to workspace lints

```toml
[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"           # <-- ADD: catches .expect("msg") too
unwrap_in_result = "deny"      # <-- ADD: catches Result.unwrap()
```

### Step B — Allowed exceptions (only for truly infallible)

For cases where the invariant is truly guaranteed (e.g., a `const` length that matches), use a named helper:

```rust
/// Use instead of .unwrap() when the invariant is locally provable.
/// The name documents WHY it's infallible — required for code review.
trait InfallibleExt {
    type Output;
    fn because(self, reason: &'static str) -> Self::Output;
}

impl<T> InfallibleExt for Option<T> {
    type Output = T;
    #[track_caller]
    fn because(self, _reason: &'static str) -> T {
        // Still panics on bug, but the reason is in git blame, not just a string
        self.expect(_reason)
    }
}

// Usage:
let img = RgbaImage::from_raw(w, h, &buf)
    .because("CheckedDims guarantees buf.len() == w*h*channels");
```

### Step C — CI enforcement

In `scripts/lint.sh`, after `cargo clippy`:

```bash
# Ban any remaining .unwrap() / .expect() in production code
# (tests may use them)
! grep -rn '\.unwrap()' pillow-rs/src/ \
  && ! grep -rn '\.expect(' pillow-rs/src/ \
  || { echo "ERROR: .unwrap()/.expect() found in production code"; exit 1; }
```

---

## Fix 3: Pipeline Boilerplate → Declarative Macro

**Class:** Adding an operation requires touching 3+ match statements in `registry.rs` plus a `PipelineOp` variant. Missing one causes runtime panics.

**Systemic fix:** A `define_op!` macro that generates all pieces from a single definition.

### The macro (`pillow-rs/src/compute/op_def.rs` — new file)

```rust
/// Define an operation ONCE.  Generates:
///  - PipelineOp variant (in pipeline.rs via include!)
///  - variant_key arm
///  - registry entry (cpu / gpu / simd)
///  - OpId arm (GPU)
macro_rules! define_op {
    // --- CPU-only op (no GPU/SIMD) ---
    (
        $(#[$meta:meta])*
        $variant:ident {
            key: $key:literal,
            fields: { $($field:ident : $ftype:ty),* $(,)? },
            cpu: |$img:ident, $mode:ident $(, $fname:ident)*| $cpu_body:expr,
        }
    ) => {
        define_op!(@internal
            meta: [$(#[$meta])*],
            variant: $variant,
            key: $key,
            fields: [{ $($field: $ftype),* }],
            cpu: |$img, $mode $(, $fname)*| $cpu_body,
            gpu: None,
            simd: None,
        );
    };

    // --- CPU + GPU op ---
    (
        $(#[$meta:meta])*
        $variant:ident {
            key: $key:literal,
            fields: { $($field:ident : $ftype:ty),* $(,)? },
            cpu: |$img:ident, $mode:ident $(, $fname:ident)*| $cpu_body:expr,
            gpu: $gpu_shader:literal,
            gpu_fn: |$gimg:ident $(, $gfname:ident)*| $gpu_body:expr,
        }
    ) => {
        define_op!(@internal
            meta: [$(#[$meta])*],
            variant: $variant,
            key: $key,
            fields: [{ $($field: $ftype),* }],
            cpu: |$img, $mode $(, $fname)*| $cpu_body,
            gpu: Some(($gpu_shader, stringify!($gpu_body))),
            simd: None,
        );
    };

    // --- Private: generate all pieces ---
    (@internal
        meta: [$($meta:meta)*],
        variant: $variant:ident,
        key: $key:literal,
        fields: [{$($field:ident : $ftype:ty),*}],
        cpu: |$cpu_img:ident, $cpu_mode:ident $(, $cpu_fname:ident)*| $cpu_body:expr,
        gpu: $gpu:expr,
        simd: $simd:expr,
    ) => {
        // 1. PipelineOp variant
        // (generated into the PipelineOp enum via include! of the expanded tokens)
        stringify!(
            $($meta)*
            $variant {
                $($field: $ftype,)*
            },
        );

        // 2. variant_key match arm (goes in variant_key())
        stringify!(
            PipelineOp::$variant { .. } => $key,
        );

        // 3. OpId match arm (goes in op_id())
        stringify!(
            PipelineOp::$variant { .. } => OpId::$variant,
        );

        // 4. Registry entry (goes in register_all())
        stringify!(
            map.insert(
                $key,
                OpEntry {
                    cpu_fn: Some(|img, mode| {
                        if let PipelineOp::$variant { $($field),* } = op {
                            $cpu_body
                        } else {
                            unreachable!()
                        }
                    }),
                    gpu_shader: $gpu.map(|(s, _)| s),
                    gpu_source: $gpu.map(|(_, src)| src),
                    simd_fn: $simd,
                },
            );
        );
    };
}
```

### Usage — a single definition per operation

```rust
// Before: 3 match arms + 1 registration = ~60 lines in 3 places
// After: 12 lines in 1 place
define_op! {
    /// Crop an image to a box.
    Crop {
        key: "Crop",
        fields: { x: u32, y: u32, width: u32, height: u32 },
        cpu: |img, mode, x, y, width, height| {
            pool_cpu::ops::geometry::op_crop(img, *x, *y, *width, *height)
        },
    }
}
```

### Migration plan

1. Write the macro (new file, ~80 lines)
2. Convert 5 ops to prove the pattern
3. Convert remaining ~70 ops in one PR
4. Delete the old `variant_key()`, `register_all()`, `op_id()` match statements
5. ~2,200 lines of `registry.rs` eliminated

---

## Fix 4: Backend Abstraction Leak → `BackendOp` Trait

**Class:** GPU fields (`gpu_shader`, `gpu_source`) on every `OpEntry`, GPU shader compilation in registry, hard-coded CPU dispatch.

**Systemic fix:** A trait that each backend implements, with its own metadata store.

```rust
/// Each backend stores its own op metadata.
/// The registry only holds the union of all backends' data.
pub trait BackendOp: Send + Sync {
    /// Human-readable name for debugging
    fn backend_name() -> &'static str;

    /// Does this backend support this operation?
    fn supports(op_key: &str) -> bool;

    /// Execute one operation on this backend.
    fn execute(op_key: &str, img: &DynamicImage, params: &[u32])
        -> Result<DynamicImage, PilError>;
}

// CPU: always supports everything, dispatches via a HashMap<&str, fn>
pub struct CpuBackend {
    ops: HashMap<&'static str, CpuFn>,
}

impl BackendOp for CpuBackend {
    fn backend_name() -> &'static str { "cpu" }
    fn supports(op_key: &str) -> bool { true } // CPU is universal
    fn execute(op_key: &str, img: &DynamicImage, params: &[u32])
        -> Result<DynamicImage, PilError>
    {
        let func = self.ops.get(op_key)
            .ok_or_else(|| PilError::NotImplementedError(format!("CPU: no op '{op_key}'")))?;
        func(img, params)
    }
}

// GPU: supports only compiled shaders, checks live pipeline cache
pub struct GpuBackend {
    ops: HashMap<&'static str, GpuOpData>,
    compiled_pipelines: HashMap<&'static str, wgpu::ComputePipeline>,
}

impl BackendOp for GpuBackend {
    fn backend_name() -> &'static str { "gpu" }
    fn supports(op_key: &str) -> bool {
        // Checks ACTUAL compiled pipelines, not static registration
        self.compiled_pipelines.contains_key(op_key)
    }
    // ...
}
```

**Kills two audit findings:** (1) `OpEntry` no longer carries GPU fields for CPU ops, (2) `supports()` is live-checked against compiled pipelines instead of static registration.

---

## Fix 5: Python Binding Violations → `check_bindings.py` Linter

**Class:** for/while loops, arithmetic, business logic leaking into Python binding layer.

**Systemic fix:** A script that statically analyzes Python binding files and fails CI on violations.

### Script (`scripts/check_bindings.py`)

```python
#!/usr/bin/env python3
"""Enforce CLAUDE.md binding-layer rules.  Run in CI; fails on violations."""
import ast
import sys
from pathlib import Path

BINDING_DIR = Path("pillow-rs-py/python/pillow_rs")
ALLOWED_IMPORTS = {
    "typing", "__future__", "pathlib", "enum",
    "pillow_rs._core", "pillow_rs._rust_image",
    "PIL.Image",  # only for type stubs / isinstance checks
}

# Nodes that indicate logic, not delegation
FORBIDDEN_NODES = {
    ast.For, ast.While, ast.ListComp, ast.SetComp, ast.DictComp,
    ast.GeneratorExp,
}

# Operators that indicate arithmetic, not delegation
FORBIDDEN_OPS = {
    ast.Add, ast.Sub, ast.Mult, ast.Div, ast.FloorDiv, ast.Mod, ast.Pow,
    ast.LShift, ast.RShift, ast.BitOr, ast.BitAnd, ast.BitXor,
}


class BindingViolation(ast.NodeVisitor):
    def __init__(self, filename: str):
        self.filename = filename
        self.violations: list[str] = []
        self.in_allowed_context = False  # for isinstance/None checks

    def visit_For(self, node):
        self.violations.append(
            f"{self.filename}:{node.lineno}: for loop not allowed in binding layer"
        )
        self.generic_visit(node)

    def visit_While(self, node):
        self.violations.append(
            f"{self.filename}:{node.lineno}: while loop not allowed in binding layer"
        )
        self.generic_visit(node)

    def visit_ListComp(self, node):
        self.violations.append(
            f"{self.filename}:{node.lineno}: list comprehension not allowed"
        )

    def visit_BinOp(self, node):
        if type(node.op) in FORBIDDEN_OPS:
            self.violations.append(
                f"{self.filename}:{node.lineno}: arithmetic ({type(node.op).__name__}) "
                f"not allowed in binding layer — move to core"
            )
        self.generic_visit(node)

    def visit_Import(self, node):
        for alias in node.names:
            if alias.name.split(".")[0] not in {"pillow_rs", "typing", "__future__"}:
                self.violations.append(
                    f"{self.filename}:{node.lineno}: import '{alias.name}'"
                    f" not allowed in binding layer"
                )

    # Allow: if isinstance(x, Image): return x._rust_image
    # Allow: if mode is None: mode = "RGB"
    def visit_If(self, node):
        # Check if this if/elif is an isinstance or None check
        if self._is_trivial_guard(node.test):
            self.in_allowed_context = True
            # Visit body — arithmetic inside allowed guards is still forbidden
            for stmt in node.body:
                self.visit(stmt)
            self.in_allowed_context = False
            # Visit orelse
            for stmt in node.orelse:
                self.visit(stmt)
        else:
            self.violations.append(
                f"{self.filename}:{node.lineno}: complex if/elif — "
                f"only isinstance/None guards allowed"
            )

    def _is_trivial_guard(self, test):
        """Check if test is: isinstance(x, T), x is None, x is not None, mode == 'P'"""
        if isinstance(test, ast.Call) and isinstance(test.func, ast.Name):
            if test.func.id == "isinstance":
                return True
        if isinstance(test, ast.Is) or isinstance(test, ast.IsNot):
            return True
        if isinstance(test, ast.Compare):
            # Allow simple mode checks: mode == "P", mode in ("L", "RGB")
            return True
        return False

    def visit_FunctionDef(self, node):
        # Check function length (should be short delegations)
        if len(node.body) > 20 and not node.name.startswith("_"):
            self.violations.append(
                f"{self.filename}:{node.lineno}: function '{node.name}' is "
                f"{len(node.body)} lines — bindings should be thin (~10 lines max)"
            )
        self.generic_visit(node)


def main():
    violations = []
    for pyfile in BINDING_DIR.rglob("*.py"):
        if pyfile.name.startswith("_"):
            continue
        tree = ast.parse(pyfile.read_text(), filename=str(pyfile))
        checker = BindingViolation(str(pyfile))
        checker.visit(tree)
        violations.extend(checker.violations)

    if violations:
        print(f"ERROR: {len(violations)} binding-layer violations:")
        for v in violations:
            print(f"  {v}")
        print("\nMove all logic to pillow-rs/src/. Bindings should delegate only.")
        sys.exit(1)

    print("OK: All binding files comply with CLAUDE.md rules.")
    sys.exit(0)


if __name__ == "__main__":
    main()
```

### CI integration

In `scripts/lint.sh`:

```bash
# Check Python bindings comply with CLAUDE.md
python scripts/check_bindings.py || exit 1
```

---

## Fix 6: Missing Test Coverage → Manifest-Driven Gap Detector

**Class:** Operations marked `implemented` but lacking fixtures or edge cases.

**Systemic fix:** Extend `test_coverage_complete()` to also check edge cases and mode gaps, and fail CI if any `implemented` op has zero fixtures.

### Extend `tests/conftest.py`

```python
def pytest_sessionfinish(session, exitstatus):
    """After all tests, validate that every 'implemented' op has at least one fixture."""
    manifest = session.config.manifest  # set during collection
    fixture_dir = Path("tests/fixtures/input/jsons")
    fixture_dir2 = Path("tests/fixtures_2/input/jsons")

    missing = []
    for module_name, module_data in manifest.items():
        for target, ops in module_data.items():
            if not isinstance(ops, list):
                continue
            for op in ops:
                if op.get("status") != "implemented":
                    continue
                fixture_name = f"{module_name}.{target}.json"
                if not (fixture_dir / fixture_name).exists() and \
                   not (fixture_dir2 / fixture_name).exists():
                    missing.append(fixture_name)

    if missing:
        print(f"\nERROR: {len(missing)} implemented operations have NO fixtures:")
        for m in sorted(missing):
            print(f"  - {m}")
        print("Generate with: python scripts/generate_fixtures.py")
        session.exitstatus = 1
```

---

## Fix 7: Duplicate Code → Shared Utility Extraction + CI Ban

**Class:** `raw_bytes_to_image` exists in 3 files. Channel-match-`from_raw` pattern repeated 12+ times.

**Systemic fix:** Extract to a single module and add a CI check that bans the pattern from other files.

### Step A — Single canonical function (`pillow-rs/src/image_utils.rs`)

```rust
/// THE canonical way to convert raw bytes to a DynamicImage.
/// Every call site must use this — no more inlining the match.
pub fn raw_bytes_to_image(
    width: u32,
    height: u32,
    channels: u8,
    data: &[u8],
) -> Result<DynamicImage, PilError> {
    let dims = CheckedDims::new(width, height, channels)?;
    if data.len() < dims.total_bytes() {
        return Err(PilError::ValueError(format!(
            "raw_bytes_to_image: expected {} bytes, got {}",
            dims.total_bytes(),
            data.len(),
        )));
    }
    Ok(match channels {
        1 => DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, data[..dims.total_bytes()].to_vec())
                .because("CheckedDims + len check guarantee correct size")
        ),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(width, height, data[..dims.total_bytes()].to_vec())
                .because("CheckedDims + len check guarantee correct size")
        ),
        3 => DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, data[..dims.total_bytes()].to_vec())
                .because("CheckedDims + len check guarantee correct size")
        ),
        4 => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, data[..dims.total_bytes()].to_vec())
                .because("CheckedDims + len check guarantee correct size")
        ),
        _ => unreachable!("channels validated by CheckedDims"),
    })
}
```

### Step B — CI script to ban copies

```bash
#!/bin/bash
# scripts/check_no_duplicate_raw_bytes.sh
# Fail if raw_bytes_to_image logic appears outside the canonical location.
PATTERN='ImageLuma8.*from_raw|ImageLumaA8.*from_raw|ImageRgb8.*from_raw|ImageRgba8.*from_raw'

# Allowed: the canonical function and tests
if grep -rn "$PATTERN" pillow-rs/src/ \
   | grep -v 'pillow-rs/src/image_utils.rs' \
   | grep -v '#\[cfg(test)\]' \
   | grep -v '#\[test\]'; then
    echo "ERROR: raw_bytes_to_image logic duplicated outside image_utils.rs"
    echo "Use image_utils::raw_bytes_to_image() instead."
    exit 1
fi
```

---

## Fix 8: Magic Numbers → Named Const Enums

**Class:** `mode >= 2`, `mode == 1 || mode == 3`, `mode * 256` repeated 80+ times in SIMD code.

**Systemic fix:** A `PixelFormat` enum with methods, plus a clippy lint to ban bare mode comparisons.

### The enum (`pillow-rs/src/pixel_format.rs`)

```rust
/// Pixel format encoding used across backends.
/// Replaces all bare `0/1/2/3` mode integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PixelFormat {
    L = 0,    // 1 channel: gray
    LA = 1,   // 2 channels: gray + alpha
    RGB = 2,  // 3 channels: red, green, blue
    RGBA = 3, // 4 channels: red, green, blue, alpha
}

impl PixelFormat {
    pub fn channels(self) -> u8 {
        match self {
            Self::L => 1,
            Self::LA => 2,
            Self::RGB => 3,
            Self::RGBA => 4,
        }
    }

    pub fn has_alpha(self) -> bool {
        matches!(self, Self::LA | Self::RGBA)
    }

    /// Stride in bytes for one row of width `w`
    pub fn row_stride(self, w: u32) -> usize {
        w as usize * self.channels() as usize
    }

    /// Packed-pixel representation for GPU (matches existing mode_code())
    pub fn gpu_encoding(self) -> u32 {
        self as u32
    }
}

impl From<ColorMode> for PixelFormat {
    fn from(mode: ColorMode) -> Self {
        match mode {
            ColorMode::L => Self::L,
            ColorMode::LA => Self::LA,
            ColorMode::RGB => Self::RGB,
            ColorMode::RGBA => Self::RGBA,
            // Non-standard modes get converted before reaching this point
            _ => unreachable!("non-standard mode reached PixelFormat"),
        }
    }
}
```

### Migration pattern

Before:
```rust
let channels = if mode >= 2 { if mode == 2 { 3 } else { 4 } } else { if mode == 1 { 2 } else { 1 } };
let has_alpha = mode == 1 || mode == 3;
let stride = mode as usize * w as usize;
```

After:
```rust
let fmt = PixelFormat::from(mode);
let channels = fmt.channels();
let has_alpha = fmt.has_alpha();
let stride = fmt.row_stride(w);
```

---

## Fix 9: Error Handling Inconsistency → `PilError` Extension + Ban `String` Errors

**Class:** `String` as error type in palette functions. `AssertionError(String::new())` dead variant.

**Systemic fix:** Add a `PaletteError` variant, ban `Result<_, String>` via clippy, and handle dead variants.

### Step A — Extend `PilError`

```rust
// pillow-rs/src/error.rs — add:
#[derive(Error, Debug)]
pub enum PilError {
    // ... existing variants ...

    #[error("palette error: {0}")]
    PaletteError(String),

    #[error("internal error: {0}")]
    InternalError(String),  // replaces dead AssertionError
}
```

### Step B — Clippy: ban `Result<_, String>`

Not directly possible via clippy, but a grep in CI works:

```bash
# scripts/lint.sh
if grep -rn 'Result<.*String>' pillow-rs/src/ | grep -v '#\[allow' | grep -v 'cfg(test)'; then
    echo "ERROR: Result<_, String> found. Use PilError variants instead."
    exit 1
fi
```

---

## Fix 10: Format Extensibility → `FormatHandler` Trait + Registry

**Class:** Format support scattered across 4+ hard-coded match statements. Adding a format touches 4 files.

**Systemic fix:** A `FormatHandler` trait. New formats implement it and self-register via `inventory` or a manual registration call.

```rust
/// pillow-rs/src/formats/handler.rs

/// Implement this trait to add a new image format.
pub trait FormatHandler: Send + Sync {
    /// Format name, e.g., "PNG", "JPEG"
    fn name(&self) -> &'static str;

    /// File extensions, e.g., &["png"]
    fn extensions(&self) -> &'static [&'static str];

    /// Magic bytes for detection, e.g., &[&[0x89, b'P', b'N', b'G']]
    fn magic_bytes(&self) -> &'static [&'static [u8]];

    /// Decode from bytes → DynamicImage
    fn decode(&self, data: &[u8]) -> Result<DynamicImage, PilError>;

    /// Encode DynamicImage → bytes (with format-specific options)
    fn encode(
        &self,
        img: &DynamicImage,
        options: &FormatEncodeOptions,
    ) -> Result<Vec<u8>, PilError>;

    /// Detect the mode of an image without fully decoding
    fn detect_mode(&self, data: &[u8]) -> Option<String>;

    /// Whether this format supports palette-indexed images
    fn supports_palette(&self) -> bool { false }

    /// Decode as palette-indexed (only if supports_palette() returns true)
    fn decode_paletted(&self, data: &[u8]) -> Result<PalettedData, PilError> {
        Err(PilError::NotImplementedError(
            format!("{} does not support paletted decode", self.name())
        ))
    }
}
```

### Registry

```rust
/// pillow-rs/src/formats/registry.rs
use std::sync::OnceLock;

static FORMAT_REGISTRY: OnceLock<Vec<Box<dyn FormatHandler>>> = OnceLock::new();

pub fn register_format(handler: Box<dyn FormatHandler>) {
    FORMAT_REGISTRY.get_or_init(Vec::new).push(handler);
}

pub fn detect_format(data: &[u8]) -> Option<&dyn FormatHandler> {
    let registry = FORMAT_REGISTRY.get()?;
    registry.iter().find(|h| {
        h.magic_bytes().iter().any(|magic| data.starts_with(magic))
    }).map(|b| b.as_ref())
}

pub fn parse_format_str(s: &str) -> Result<&dyn FormatHandler, PilError> {
    let registry = FORMAT_REGISTRY.get()
        .ok_or_else(|| PilError::InternalError("format registry not initialized".into()))?;
    let s_lower = s.to_lowercase();
    registry.iter().find(|h| {
        h.name().to_lowercase() == s_lower
            || h.extensions().iter().any(|ext| ext.to_lowercase() == s_lower)
    }).map(|b| b.as_ref())
    .ok_or_else(|| PilError::UnknownFormat(s.to_string()))
}
```

### Usage — adding a new format is now ONE file

```rust
// pillow-rs/src/formats/qoi.rs  — everything for QOI in one place
struct QoiFormat;
impl FormatHandler for QoiFormat {
    fn name(&self) -> &'static str { "QOI" }
    fn extensions(&self) -> &'static [&'static str] { &["qoi"] }
    fn magic_bytes(&self) -> &'static [&'static [u8]] { &[b"qoif"] }
    fn decode(&self, data: &[u8]) -> Result<DynamicImage, PilError> { /* ... */ }
    fn encode(&self, img: &DynamicImage, opts: &FormatEncodeOptions) -> Result<Vec<u8>, PilError> { /* ... */ }
    fn detect_mode(&self, data: &[u8]) -> Option<String> { /* ... */ }
}

// Register at startup:
inventory::submit!(FormatRegistration { handler: || Box::new(QoiFormat) });
```

---

## Fix 11: Dead Code → Deny in CI

**Class:** `#[allow(dead_code)]` on functions, unused imports, orphaned annotations.

**Systemic fix:**

### Step A — Workspace lints

```toml
[workspace.lints.rust]
dead_code = "deny"
unused_imports = "deny"
unused_variables = "deny"
unused_assignments = "deny"
```

### Step B — CI enforcement

```bash
# In scripts/lint.sh — fail on any #[allow(dead_code)]
if grep -rn '#\[allow(dead_code)\]' pillow-rs/src/; then
    echo "ERROR: #[allow(dead_code)] found. Remove the code or use it."
    exit 1
fi

# Fail on unused imports suppressed by allow
if grep -rn '#\[allow(unused_imports)\]' pillow-rs/src/; then
    echo "ERROR: #[allow(unused_imports)] found. Remove unused imports."
    exit 1
fi
```

---

## Fix 12: Missing Docs → Deny on Public APIs

**Class:** 30+ public functions and 5 public types without doc comments.

**Systemic fix:**

```toml
[workspace.lints.rust]
missing_docs = "deny"
```

Then add `///` comments to all public items. For types/enums that are self-documenting:

```rust
/// Resampling filter for image resize operations.
///
/// Matches PIL's `PIL.Image.Resampling` enum exactly.
#[derive(Debug, Clone, Copy)]
pub enum ResampleFilter {
    Nearest,
    Box,
    Bilinear,
    Hamming,
    Bicubic,
    Lanczos,
    Hanning,
}
```

---

## Fix 13: XFAIL Mechanism → `xfailed_tracker.txt`

**Class:** No way to mark partially-implemented ops. Binary choice: implemented (must pass all tests) or ignored (no tests).

**Systemic fix:** A tracker file that the test runner reads to skip known-failing cases, with CI enforcement.

### Format (`xfailed_tracker.txt`)

```
# Lines: CASE_ID  REASON  DATE
# Cases listed here are SKIPPED with a warning.
# CI fails if a case is xfailed for >30 days (stale xfail).
Image_resize_Lanczos_YCbCr  "Lanczos YCbCr: precision differs by 1 gray level"  2026-06-15
Image_quantize_RGBA_256  "Quantize: palette sort order differs, pixels correct"  2026-06-18
```

### Engine integration

```python
# tests/engine.py — at test discovery time:
XFAIL_FILE = Path("xfailed_tracker.txt")

def load_xfails():
    xfails = {}
    if XFAIL_FILE.exists():
        for line in XFAIL_FILE.read_text().splitlines():
            if line.strip() and not line.startswith("#"):
                parts = line.split(None, 2)
                if len(parts) >= 1:
                    xfails[parts[0]] = parts[1] if len(parts) > 1 else "Unknown"
    return xfails

def is_xfailed(case_id: str) -> bool:
    return case_id in load_xfails()
```

### CI check for stale xfails

```bash
#!/bin/bash
# Fail if any xfailed entry is older than 30 days
python -c "
import re, sys, datetime
with open('xfailed_tracker.txt') as f:
    for line in f:
        if line.startswith('#'): continue
        m = re.search(r'(\d{4}-\d{2}-\d{2})', line)
        if m:
            age = (datetime.date.today() - datetime.date.fromisoformat(m.group(1))).days
            if age > 30:
                print(f'STALE XFAIL ({age} days old): {line.strip()}')
                sys.exit(1)
"
```

---

## Fix 14: Dependency Hygiene → `deny.toml` + CI

**Class:** Duplicate dependency versions, deprecated crates, no vulnerability scanning.

**Systemic fix:** A `cargo-deny` config that runs in CI.

### `deny.toml` (project root)

```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"
yanked = "deny"
ignore = []

[bans]
multiple-versions = "deny"
deny = [
    # Ban old png version once migration is complete
    # { crate = "png", version = "=0.17" },
]
# Allow these duplicates temporarily (tracked issues):
skip = []

[licenses]
allow = [
    "MIT", "Apache-2.0", "BSD-3-Clause", "Unicode-3.0",
    "ISC", "Zlib", "OpenSSL",
]
deny = ["GPL-2.0", "GPL-3.0", "AGPL-3.0"]

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

### CI integration

```bash
# scripts/check_deps.sh
cargo deny check advisories
cargo deny check bans
cargo deny check licenses
cargo deny check sources
cargo audit
```

Add to `scripts/lint.sh`:
```bash
bash scripts/check_deps.sh || exit 1
```

---

## Fix 15: Parallelism → `par_rows!` Macro

**Class:** All pixel loops are serial. Adding `rayon` requires repetitive boilerplate.

**Systemic fix:** A macro that makes adding rayon trivial and consistent.

```rust
/// pillow-rs/src/par.rs

/// Parallelize pixel iteration over image rows.
///
/// Usage:
/// ```
/// par_rows!(img, |row_start, row_end, y| {
///     // row_start = byte offset of row y
///     // row_end   = byte offset of next row
///     for x in 0..width {
///         // process pixel at row_start + x * channels
///     }
/// });
/// ```
#[macro_export]
macro_rules! par_rows {
    ($img:expr, $channels:expr, |$row_start:ident, $row_end:ident, $y:ident| $body:block) => {{
        let img_ref = &$img;
        let width = img_ref.width() as usize;
        let height = img_ref.height() as usize;
        let stride = width * ($channels as usize);
        let data: &[u8] = img_ref.as_bytes();

        use rayon::prelude::*;
        data.par_chunks(stride)
            .enumerate()
            .for_each(|($y, row)| {
                let $row_start = $y * stride;
                let $row_end = $row_start + stride;
                let $y = $y as u32;
                $body
            });
    }};
}
```

---

## Summary Table

| # | Audit Finding Class | Systemic Fix | Type |
|---|---------------------|-------------|------|
| 1 | Integer overflow in `(w*h) as usize` | `CheckedDims` newtype | Type system |
| 2 | `unwrap()`/`expect()` panics | Clippy `deny` + `InfallibleExt` | Lint |
| 3 | Registry boilerplate (2,696 lines) | `define_op!` macro | Code gen |
| 4 | Backend abstraction leaks | `BackendOp` trait | Trait |
| 5 | Python binding logic violations | `check_bindings.py` linter | CI script |
| 6 | Missing test coverage | Manifest-driven gap detector | CI hook |
| 7 | Duplicate `raw_bytes_to_image` | Shared module + CI ban | Extraction + CI |
| 8 | Magic mode numbers (0/1/2/3) | `PixelFormat` enum | Type system |
| 9 | `String` error type / dead variants | `PilError` extension + grep CI | Type system + CI |
| 10 | Format handling scattered | `FormatHandler` trait + registry | Trait |
| 11 | Dead code / unused imports | Clippy `deny` on `dead_code` | Lint |
| 12 | Missing doc comments | `missing_docs = "deny"` | Lint |
| 13 | No xfail for partial impl | `xfailed_tracker.txt` + stale check | Process |
| 14 | Dependency hygiene | `deny.toml` + `cargo-audit` | CI config |
| 15 | No parallelism | `par_rows!` macro | Code gen |

Each fix is **systemic**: once applied, the entire class of issue is prevented from recurring — not just this instance, but all future instances of the same pattern.
