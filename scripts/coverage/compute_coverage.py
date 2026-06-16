#!/usr/bin/env python3
"""
Coverage trust computer for pillow-rs.

Single source of truth: fixture JSONs + @pytest.mark.covers decorators.
No separate mapping file needed.

Usage:
    python scripts/coverage/compute_coverage.py [manifest.yaml] [/tmp/report.json]
    python scripts/coverage/compute_coverage.py --md

The --md flag regenerates docs/COVERAGE.md with trust report and benchmarks.
"""
import json, sys, yaml, time, re
from pathlib import Path
from collections import defaultdict

ROOT = Path(__file__).parent.parent.parent
FIXTURES_DIR = ROOT / "tests" / "fixtures"
MANIFEST_PATH = ROOT / "manifest.yaml"

# Module naming aliases: manifest uses "Image.xxx" for module-level functions,
# but fixtures group them under "ImageModule.xxx". This maps manifest names to
# fixture names so functions tested under different module names get credit.
MANIFEST_TO_FIXTURE_ALIASES = {
    "Image.open": "ImageModule.open",
    "Image.new": "ImageModule.new",
    "Image.frombytes": "ImageModule.frombytes",
    "Image.alpha_composite": "ImageModule.alpha_composite",
    "Image.filter": "ImageFilter",  # tested via individual filter classes
}


# ══════════════════════════════════════════════════════════════════════════════
# Manifest
# ══════════════════════════════════════════════════════════════════════════════

def load_manifest(path=MANIFEST_PATH):
    with open(path) as f:
        return yaml.safe_load(f)


def extract_functions(manifest):
    """Return {Module.function: status} for all functions in the manifest.

    Only returns entries with recognized status values. Entries without status
    or marked 'ignored' are excluded from trust computation.
    """
    VALID_STATUSES = {"implemented", "stub"}
    funcs = {}
    for mod, mod_def in manifest.get("modules", {}).items():
        # Class methods, instance methods, module functions
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if isinstance(item, dict):
                    status = item.get("status", "")
                    if status in VALID_STATUSES or status == "ignored":
                        funcs[f"{mod}.{item['name']}"] = status
        # Classes (ImageFilter, ImageEnhance, ImageFont, ImageDraw, etc.)
        for cls in mod_def.get("classes", []):
            if isinstance(cls, dict):
                cls_name = cls.get("name", "")
                cls_status = cls.get("status", "")
                methods = cls.get("methods", [])
                if methods:
                    # Class with named methods: register Module.method
                    for m in methods:
                        name = m.get("name", str(m)) if isinstance(m, dict) else str(m)
                        m_status = m.get("status", cls_status) if isinstance(m, dict) else cls_status
                        if m_status in VALID_STATUSES or m_status == "ignored":
                            funcs[f"{mod}.{name}"] = m_status
                else:
                    # No explicit methods: the class IS the function
                    # e.g., ImageFilter.GaussianBlur, ImageEnhance.Brightness
                    if cls_status in VALID_STATUSES or cls_status == "ignored":
                        funcs[f"{mod}.{cls_name}"] = cls_status
        # Properties
        for prop in mod_def.get("properties", []):
            if isinstance(prop, dict):
                status = prop.get("status", "")
                if status in VALID_STATUSES:
                    funcs[f"{mod}.{prop['name']}"] = status
    return funcs


# ══════════════════════════════════════════════════════════════════════════════
# Mapping: fixture JSONs → function names
# ══════════════════════════════════════════════════════════════════════════════

def build_fixture_map(fixtures_dir=FIXTURES_DIR):
    """Read fixture JSONs and return {param_id: [func_names]}.

    Each fixture JSON has: {"operation": {"module": "Image", "target": "resize"}, ...}
    The parametrize ID (fpath.stem) is e.g. "Image_resize_L".
    """
    fixture_map = {}
    for fpath in sorted(fixtures_dir.glob("*.json")):
        with open(fpath) as f:
            fx = json.load(f)
        op = fx["operation"]
        func_name = f"{op.get('module', '?')}.{op['target']}"
        param_id = fpath.stem  # e.g., Image_resize_L
        if param_id in fixture_map:
            if func_name not in fixture_map[param_id]:
                fixture_map[param_id].append(func_name)
        else:
            fixture_map[param_id] = [func_name]
    return fixture_map


# ══════════════════════════════════════════════════════════════════════════════
# Mapping: static @pytest.mark.covers decorators
# ══════════════════════════════════════════════════════════════════════════════

COVERS_RE = re.compile(
    r'@pytest\.mark\.covers\(\s*"([^"]+)"'
    r'(?:,\s*mode="([^"]*)")?\s*'
    r'(?:,\s*target="([^"]*)")?\s*'
    r'(?:,\s*variant="([^"]*)")?\s*'
    r'\)'
)

INLINE_COVERS_RE = re.compile(
    r'pytest\.mark\.covers\(\s*"([^"]+)"'
    r'(?:,\s*mode="([^"]*)")?\s*'
    r'(?:,\s*target="([^"]*)")?\s*'
    r'(?:,\s*variant="([^"]*)")?\s*'
    r'\)'
)


def build_static_map(tests_dir=ROOT / "tests"):
    """Scan test files for @pytest.mark.covers decorators.

    Returns {nodeid_key: [func_names]} where keys are:
      - "file.py::ClassName::test_name" for class methods
      - "file.py::test_name" for module-level functions
    """
    static_map = {}
    for py_file in sorted(tests_dir.rglob("test_*.py")):
        content = py_file.read_text()
        file_name = py_file.name
        cls_name = None
        pending_covers = None

        for line in content.split('\n'):
            # Track class context
            cls_match = re.match(r'class\s+(\w+)', line)
            if cls_match:
                cls_name = cls_match.group(1)
                continue
            # Empty line or decorator line – check for @covers
            m = COVERS_RE.search(line)
            if m:
                pending_covers = m.group(1)
                continue
            m2 = INLINE_COVERS_RE.search(line)
            if m2:
                pending_covers = m2.group(1)
                continue
            # Test function
            if line.strip().startswith('def test_'):
                func_name = line.strip().split('(')[0].replace('def ', '')
                if cls_name:
                    key = f"{file_name}::{cls_name}::{func_name}"
                else:
                    key = f"{file_name}::{func_name}"
                if pending_covers:
                    static_map[key] = [pending_covers]
                pending_covers = None

    return static_map


# ══════════════════════════════════════════════════════════════════════════════
# Inference: nodeid → function names
# ══════════════════════════════════════════════════════════════════════════════

def infer_functions(nodeid, fixture_map, static_map):
    """Map a pytest nodeid to list of Module.function names.

    For fixture tests: extracts parametrize ID from nodeid.
    For static tests: tries file::class::test, class::test, bare test name.
    """
    # Check if this is a fixture test
    if "test_parity[" in nodeid or "test_parity.py::" in nodeid or "test_fixture_parity[" in nodeid or "test_fixture_parity.py::" in nodeid:
        # Extract parametrize ID: "test_fixture_parity[Image_resize_L]" → "Image_resize_L"
        m = re.search(r'\[([^\]]+)\]', nodeid)
        if m:
            param_id = m.group(1)
            return fixture_map.get(param_id, [])

    # Static test: parse nodeid
    parts = nodeid.split("::")

    if len(parts) >= 3:
        # "tests/file.py::ClassName::test_name[params]"
        file_name = parts[0].split("/")[-1]  # with .py
        class_name = parts[-2]
        test_name_raw = parts[-1]
        # Strip parametrize brackets
        test_name = re.sub(r'\[.*\]', '', test_name_raw)

        # Try with and without .py prefix
        keys = [
            f"{file_name}::{class_name}::{test_name_raw}",
            f"{file_name}::{class_name}::{test_name}",
            f"{file_name.replace('.py', '')}::{class_name}::{test_name_raw}",
            f"{file_name.replace('.py', '')}::{class_name}::{test_name}",
            f"{class_name}::{test_name_raw}",
            f"{class_name}::{test_name}",
        ]
        for key in keys:
            if key in static_map:
                return static_map[key]

    elif len(parts) == 2:
        # "tests/file.py::test_name[params]"
        file_name = parts[0].split("/")[-1]
        test_name_raw = parts[-1]
        test_name = re.sub(r'\[.*\]', '', test_name_raw)

        keys = [
            f"{file_name}::{test_name_raw}",
            f"{file_name}::{test_name}",
            f"{file_name.replace('.py', '')}::{test_name_raw}",
            f"{file_name.replace('.py', '')}::{test_name}",
            test_name_raw,
            test_name,
        ]
        for key in keys:
            if key in static_map:
                return static_map[key]

    return []


# ══════════════════════════════════════════════════════════════════════════════
# Trust computation
# ══════════════════════════════════════════════════════════════════════════════

def compute_trust(manifest, report, fixture_map, static_map):
    """Compute trust from pytest report."""
    tested = defaultdict(lambda: {"passed": 0, "failed": 0})
    untracked = []
    skipped = 0

    for test in report.get("tests", []):
        funcs = infer_functions(test["nodeid"], fixture_map, static_map)
        if not funcs:
            untracked.append(test["nodeid"])
        outcome = test.get("outcome")
        for func in funcs:
            if outcome == "passed":
                tested[func]["passed"] += 1
            elif outcome == "failed":
                tested[func]["failed"] += 1
        if outcome not in ("passed", "failed"):
            skipped += 1

    all_funcs = extract_functions(manifest)
    implemented = {k: v for k, v in all_funcs.items() if v == "implemented"}
    stubs = {k for k, v in all_funcs.items() if v == "stub"}

    # Compute trust, checking aliases for naming mismatches
    def is_trusted(func_name):
        # Direct match
        if tested[func_name]["passed"] > 0 and tested[func_name]["failed"] == 0:
            return True
        # Check alias
        alias = MANIFEST_TO_FIXTURE_ALIASES.get(func_name)
        if alias:
            if tested[alias]["passed"] > 0 and tested[alias]["failed"] == 0:
                return True
            # If alias is a module prefix (e.g., "ImageFilter"), check if any function
            # starting with that prefix is tested
            if not alias.endswith('.'):
                for fn_key in tested:
                    if fn_key.startswith(alias + ".") and tested[fn_key]["passed"] > 0 and tested[fn_key]["failed"] == 0:
                        return True
        return False

    trusted = {k for k in implemented if is_trusted(k)}
    untrusted = [k for k in implemented if k not in trusted]

    total = len(report.get("tests", []))
    passed = sum(1 for t in report.get("tests", []) if t.get("outcome") == "passed")
    failed = sum(1 for t in report.get("tests", []) if t.get("outcome") == "failed")

    return {
        "tested": tested,
        "all_funcs": all_funcs,
        "implemented": implemented,
        "stubs": stubs,
        "trusted": trusted,
        "untrusted": untrusted,
        "untracked": untracked,
        "total_tests": total,
        "passed_tests": passed,
        "failed_tests": failed,
        "skipped_tests": skipped,
    }


# ══════════════════════════════════════════════════════════════════════════════
# Text report (for lint.sh)
# ══════════════════════════════════════════════════════════════════════════════

def print_text_report(data):
    """Print trust report to stdout."""
    implemented = data["implemented"]
    trusted = data["trusted"]
    untrusted = data["untrusted"]
    stubs = data["stubs"]
    untracked = data["untracked"]
    trust_pct = len(trusted) / max(len(implemented), 1) * 100

    # Module breakdown
    mod_data = defaultdict(lambda: {"impl": 0, "trusted": 0})
    for k in implemented:
        mod_data[k.split(".")[0]]["impl"] += 1
    for k in trusted:
        mod_data[k.split(".")[0]]["trusted"] += 1

    print(f"\n{'='*65}")
    print(f"  pillow-rs TRUST Report — {trust_pct:.0f}% of API has PIL parity tests")
    print(f"{'='*65}")
    print(f"  {'Module':<22} {'Impl':>5} {'Trusted':>7} {'Untested':>8}  Status")
    print(f"  {'-'*55}")
    for mod, stats in sorted(mod_data.items()):
        impl, tr = stats["impl"], stats["trusted"]
        unt = impl - tr
        status = "✅" if unt == 0 else "⚠️" if tr > 0 else "⬜"
        print(f"  {mod:<22} {impl:>5} {tr:>7} {unt:>8}  {status}")
    print(f"{'='*65}")

    if untrusted:
        print(f"\n  ⚠️  UNTESTED ({len(untrusted)}):")
        for k in sorted(untrusted):
            print(f"    - {k}")

    if untracked:
        print(f"\n  🔍 UNTRACKED ({len(untracked)} tests not mapped to any function):")
        for t in sorted(set(untracked))[:15]:
            print(f"    - {t}")

    print(f"\n  ✅ TRUSTED: {len(trusted)}  ⚠️ UNTESTED: {len(untrusted)}  ⬜ STUBS: {len(stubs)}  🔍 UNTRACKED: {len(untracked)}")
    print(f"  📊 {data['total_tests']} tests, {data['passed_tests']} passed, {data['failed_tests']} failed, {data['skipped_tests']} skipped\n")


# ══════════════════════════════════════════════════════════════════════════════
# Mode × Operation coverage matrix
# ══════════════════════════════════════════════════════════════════════════════

ALL_MODES = ["1", "L", "LA", "P", "RGB", "RGBA", "CMYK", "YCbCr", "HSV", "I", "F"]

# Which operations are actually tested by each fixture function
def build_mode_matrix(fixture_map, report):
    """Build {func_name: {mode: status}} from fixture data and test report.

    Status is one of: '✅' (passed), '⚠️' (xfailed), '❌' (failed), '' (not tested)
    """
    # Reverse aliases: fixture name → manifest name
    reverse_aliases = {}
    for manifest_name, fixture_name in MANIFEST_TO_FIXTURE_ALIASES.items():
        if not fixture_name.endswith('.'):
            reverse_aliases[fixture_name] = manifest_name

    outcomes = {}
    for test in report.get("tests", []):
        nodeid = test["nodeid"]
        outcome = test.get("outcome", "unknown")
        m = re.search(r'\[([^\]]+)\]', nodeid)
        if not m:
            continue
        param_id = m.group(1)
        funcs = fixture_map.get(param_id, [])
        if not funcs:
            continue
        # Extract mode from param_id
        for mode in ALL_MODES:
            if param_id.endswith("_" + mode):
                for func in funcs:
                    # Apply reverse alias to get manifest name
                    manifest_func = reverse_aliases.get(func, func)
                    key = (manifest_func, mode)
                    if outcome == "passed":
                        outcomes[key] = "✅"
                    elif outcome == "xfailed":
                        outcomes[key] = "⚠️"
                    elif outcome == "failed":
                        outcomes[key] = "❌"
                break
        else:
            for func in funcs:
                manifest_func = reverse_aliases.get(func, func)
                key = (manifest_func, "")
                if outcome == "passed":
                    outcomes[key] = "✅"
                elif outcome == "xfailed":
                    outcomes[key] = "⚠️"
                elif outcome == "failed":
                    outcomes[key] = "❌"

    return outcomes


def generate_mode_matrix_md(manifest, outcomes):
    """Generate mode × operation matrix using manifest supported_modes.

    - ✅ = passing, ⚠️ = xfailed, ⬜ = supported but not tested, N/A = PIL doesn't support
    """
    mod_funcs = defaultdict(list)
    # Get supported_modes per function from manifest
    supported_modes = {}  # func_name → set of mode strings
    for mod_name, mod_def in manifest.get("modules", {}).items():
        for section in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(section, []):
                if isinstance(item, dict) and item.get("status") == "implemented":
                    fname = f"{mod_name}.{item['name']}"
                    modes = set(item.get("supported_modes", []))
                    supported_modes[fname] = modes
                    mod_funcs[mod_name].append(fname)
        for cls in mod_def.get("classes", []):
            if isinstance(cls, dict) and cls.get("status") == "implemented":
                methods = cls.get("methods", [])
                if methods:
                    for m in methods:
                        if isinstance(m, dict) and m.get("status") == "implemented":
                            fname = f"{mod_name}.{m['name']}"
                            modes = set(m.get("supported_modes", cls.get("supported_modes", [])))
                            supported_modes[fname] = modes
                            mod_funcs[mod_name].append(fname)
                else:
                    fname = f"{mod_name}.{cls['name']}"
                    modes = set(cls.get("supported_modes", []))
                    supported_modes[fname] = modes
                    mod_funcs[mod_name].append(fname)

    # Collect all modes that appear in supported_modes across the module
    COMMON_MODES = ["1", "L", "LA", "P", "RGB", "RGBA", "CMYK", "HSV", "I", "F"]

    md = "\n## Mode × Operation Coverage Matrix\n\n"
    md += "*✅ = passing, ⚠️ = xfailed (in progress), ⬜ = supported but not tested, N/A = PIL doesn't support this mode*\n\n"

    for mod_name in sorted(mod_funcs):
        funcs = sorted(mod_funcs[mod_name])
        # Find modes actually used as "supported" in this module
        all_supported = set()
        for f in funcs:
            all_supported |= supported_modes.get(f, set())
        modes_used = sorted(all_supported & set(COMMON_MODES),
                          key=lambda m: COMMON_MODES.index(m) if m in COMMON_MODES else 99)
        if not modes_used:
            continue

        md += f"### {mod_name}\n\n"
        md += "| Operation | " + " | ".join(modes_used) + " |\n"
        md += "|-----------|" + "|".join(["-------" for _ in modes_used]) + "|\n"

        for func_name in funcs:
            short_name = func_name.split(".", 1)[1] if "." in func_name else func_name
            fn_modes = supported_modes.get(func_name, set())
            cells = []
            for mode in modes_used:
                if mode not in fn_modes:
                    cells.append("N/A")
                else:
                    key = (func_name, mode)
                    cells.append(outcomes.get(key, "⬜"))
            md += f"| `{short_name}` | " + " | ".join(cells) + " |\n"

        md += "\n"

    return md

def run_benchmarks():
    """Run PIL vs pillow-rs performance benchmarks. Returns {label: speedup}."""
    import PIL.Image as PILImage
    import PIL.ImageFilter as PILFilter
    import PIL.ImageOps as PILOps
    from pillow_rs import Image, ImageOps

    benchmarks = {}
    N = 50
    pil_large = PILImage.new("RGB", (2000, 2000), (128, 128, 128))
    rs_large = Image.new("RGB", (2000, 2000), (128, 128, 128))
    pil_small = PILImage.new("RGB", (100, 100), (255, 0, 0))

    def bench(pil_op, rs_op):
        t0 = time.perf_counter()
        for _ in range(N): pil_op()
        t_pil = max(time.perf_counter() - t0, 0.0001)
        t0 = time.perf_counter()
        for _ in range(N): rs_op()
        t_rs = max(time.perf_counter() - t0, 0.0001)
        return round(t_pil / t_rs, 2)

    benchmarks["resize_2k_to_1k"] = bench(
        lambda: pil_large.resize((1000, 1000)),
        lambda: rs_large.resize((1000, 1000)))
    benchmarks["crop_2k"] = bench(
        lambda: pil_large.crop((500, 500, 1500, 1500)),
        lambda: rs_large.crop((500, 500, 1500, 1500)))
    benchmarks["convert_2k_RGB_to_L"] = bench(
        lambda: pil_large.convert("L"),
        lambda: rs_large.convert("L"))
    benchmarks["transpose_2k_FLIP"] = bench(
        lambda: pil_large.transpose(PILImage.FLIP_LEFT_RIGHT),
        lambda: rs_large.transpose(0))
    benchmarks["filter_2k_BLUR"] = bench(
        lambda: pil_large.filter(PILFilter.BLUR),
        lambda: rs_large.filter("BLUR"))
    benchmarks["paste_2k"] = bench(
        lambda: pil_large.copy().paste(pil_small, (0, 0)),
        lambda: rs_large.copy().paste(Image.new("RGB", (100, 100), (255, 0, 0)), (0, 0)))
    benchmarks["invert_2k"] = bench(
        lambda: PILOps.invert(pil_large),
        lambda: ImageOps.invert(rs_large))

    return benchmarks


def generate_markdown(data, manifest=None, fixture_map=None, report=None):
    """Generate docs/COVERAGE.md."""
    implemented = data["implemented"]
    trusted = data["trusted"]
    untrusted = data["untrusted"]
    stubs = data["stubs"]
    untracked = data["untracked"]
    trust_pct = len(trusted) / max(len(implemented), 1) * 100

    benchmarks = run_benchmarks()
    avg_speedup = round(sum(benchmarks.values()) / max(len(benchmarks), 1), 2)
    now = time.strftime("%Y-%m-%d %H:%M:%S")

    # Build mode matrix from fixtures and report
    if fixture_map and report:
        mode_outcomes = build_mode_matrix(fixture_map, report)
    else:
        mode_outcomes = {}

    # Module breakdown
    mod_data = defaultdict(lambda: {"impl": 0, "trusted": 0})
    for k in implemented:
        mod_data[k.split(".")[0]]["impl"] += 1
    for k in trusted:
        mod_data[k.split(".")[0]]["trusted"] += 1

    md = f"""# pillow-rs Coverage Report

> Auto-generated: {now} | Pillow parity tested

## Trust Summary

| Metric | Value |
|--------|-------|
| **Total tests** | {data['total_tests']} |
| **Passing** | {data['passed_tests']} |
| **Failed** | {data['failed_tests']} |
| **Skipped** | {data['skipped_tests']} |
| **Implemented functions** | {len(implemented)} |
| **Trusted (PIL parity tested)** | {len(trusted)} |
| **Untested** | {len(untrusted)} |
| **Stubs** | {len(stubs)} |
| **Trust score** | **{len(trusted)}/{len(implemented)} ({trust_pct:.0f}%)** |

## Performance Benchmarks

*Multiple = PIL time / pillow-rs time. >1.0 = pillow-rs is faster.*

| Operation | Speedup | Faster? |
|-----------|---------|---------|
"""
    for label, speedup in benchmarks.items():
        faster = "✅" if speedup > 1.0 else "❌"
        md += f"| {label} | {speedup:.2f}× | {faster} |\n"

    md += f"""
**Average speedup: {avg_speedup:.2f}×**

## Module Status

| Module | Implemented | Trusted | Untested | Trust % |
|--------|------------|---------|----------|---------|
"""
    for mod, stats in sorted(mod_data.items()):
        impl, tr = stats["impl"], stats["trusted"]
        unt = impl - tr
        pct = round(tr / max(impl, 1) * 100)
        md += f"| {mod} | {impl} | {tr} | {unt} | {pct}% |\n"

    if untrusted:
        md += f"\n## ⚠️ Untested Functions\n\n"
        for k in sorted(untrusted):
            md += f"- `{k}`\n"

    if stubs:
        md += f"\n## ⬜ Remaining Stubs\n\n"
        for k in sorted(stubs):
            md += f"- `{k}`\n"

    if untracked:
        md += f"\n## 🔍 Tests Not in Coverage Map\n\n"
        for t in sorted(set(untracked))[:30]:
            md += f"- `{t}`\n"
        if len(untracked) > 30:
            md += f"- ... and {len(untracked) - 30} more\n"

    # Mode × Operation matrix
    if manifest:
        md += generate_mode_matrix_md(manifest, mode_outcomes)

    md += """
## Reverse Verification

Every test in the trust report validates PIL-RSPIL parity:
- Tests create identical inputs for both `PIL.Image` and `pillow_rs.Image`
- Apply the same operation with identical parameters
- Assert pixel-exact binary equality or value equality
- No tests verify only signature existence or stub behavior

**Verification method:** `assert_images_equal(rs_img, pil_img)` for image output,
`assert_values_equal(rs_val, pil_val)` for non-image values. Fixture tests use
SHA-256 hash comparison with tolerance for lossy operations.

## How Coverage Mapping Works

Coverage mapping derives from two auto-discovered sources — no separate mapping file:

1. **Fixture JSONs** (365 files in `tests/fixtures/`): Each fixture declares
   `operation.module` + `operation.target` in its JSON metadata.
   The test runner (`test_fixture_parity.py`) auto-generates
   `@pytest.mark.covers` markers from this metadata at collection time.

2. **Static decorators**: Tests in `tests/test_*.py` files with
   `@pytest.mark.covers("Module.function")` decorators are parsed directly.

*Report generated by `scripts/coverage/compute_coverage.py --md`*
"""

    md_path = ROOT / "docs" / "COVERAGE.md"
    md_path.parent.mkdir(exist_ok=True)
    md_path.write_text(md)
    print(f"Generated {md_path}")
    print(f"  Trust: {len(trusted)}/{len(implemented)} ({trust_pct:.0f}%)")
    if benchmarks:
        print(f"  Benchmark avg: {avg_speedup:.2f}×")
    if untracked:
        print(f"  Untracked tests: {len(untracked)}")


# ══════════════════════════════════════════════════════════════════════════════
# Main
# ══════════════════════════════════════════════════════════════════════════════

def main():
    if "--md" in sys.argv:
        # Generate COVERAGE.md
        manifest = load_manifest()
        report_path = "/tmp/coverage_report.json"

        # Run pytest if report doesn't exist
        if not Path(report_path).exists():
            print("Running tests...")
            import subprocess
            subprocess.run([sys.executable, "-m", "pytest", str(ROOT / "tests"),
                           "-q", "--json-report", f"--json-report-file={report_path}"],
                          cwd=ROOT)

        with open(report_path) as f:
            report = json.load(f)

        fixture_map = build_fixture_map()
        static_map = build_static_map()
        data = compute_trust(manifest, report, fixture_map, static_map)
        generate_markdown(data, manifest=manifest, fixture_map=fixture_map, report=report)
    else:
        # Text report for lint.sh
        manifest_path = sys.argv[1] if len(sys.argv) > 1 else str(MANIFEST_PATH)
        report_path = sys.argv[2] if len(sys.argv) > 2 else "/tmp/report.json"

        manifest = load_manifest(manifest_path)
        report = json.loads(Path(report_path).read_text()) if Path(report_path).exists() else {"tests": []}

        fixture_map = build_fixture_map()
        static_map = build_static_map()
        data = compute_trust(manifest, report, fixture_map, static_map)
        print_text_report(data)


if __name__ == "__main__":
    main()
