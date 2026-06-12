#!/usr/bin/env python3
"""Trust-based coverage. Reads test→function mapping from coverage_map.json."""
import json, sys, yaml
from pathlib import Path
from collections import defaultdict

ROOT = Path(__file__).parent.parent.parent

def load_manifest(path):
    with open(path) as f: return yaml.safe_load(f)

def load_report(path):
    with open(path) as f: return json.load(f)

def load_func_map():
    with open(ROOT / "scripts" / "coverage_map.json") as f:
        return json.load(f)

def infer_functions(test, func_map):
    nodeid = test.get("nodeid", "")
    parts = nodeid.split("::")
    if len(parts) >= 3:
        test_name = f"{parts[-2]}::{parts[-1]}"
        file_name = parts[0].split("/")[-1].replace(".py", "")
        file_key = f"{file_name}::{test_name}"
        # Try file-qualified first, then class-qualified, then plain
        return (func_map.get(file_key) or func_map.get(test_name) or
                func_map.get(parts[-1], []))
    else:
        test_name = parts[-1] if parts else ""
        file_name = parts[0].split("/")[-1].replace(".py", "") if len(parts) >= 2 else ""
        file_key = f"{file_name}::{test_name}" if file_name else ""
        return (func_map.get(file_key) or func_map.get(test_name, []))

def extract_all(manifest):
    funcs = {}
    for mod, mod_def in manifest.get("modules", {}).items():
        for key in ["class_methods", "methods", "functions"]:
            for item in mod_def.get(key, []):
                if isinstance(item, dict):
                    funcs[f"{mod}.{item['name']}"] = item.get("status", "stub")
        for cls in mod_def.get("classes", []):
            if isinstance(cls, dict):
                n = cls.get("name", "")
                ms = cls.get("methods", [])
                for m in ms:
                    name = m.get("name", str(m)) if isinstance(m, dict) else str(m)
                    funcs[f"{mod}.{name}"] = cls.get("status", "stub")
    return funcs

def main():
    manifest_path = sys.argv[1] if len(sys.argv) > 1 else str(ROOT / "manifest.yaml")
    report_path = sys.argv[2] if len(sys.argv) > 2 else "/tmp/report.json"

    manifest = load_manifest(manifest_path)
    tests = load_report(report_path) if Path(report_path).exists() else {"tests": []}
    func_map = load_func_map()

    tested = defaultdict(lambda: {"passed": 0, "failed": 0})
    untracked = []
    for test in tests.get("tests", []):
        funcs = infer_functions(test, func_map)
        if not funcs:
            untracked.append(test["nodeid"])
        for func in funcs:
            if test.get("outcome") == "passed":
                tested[func]["passed"] += 1
            else:
                tested[func]["failed"] += 1

    all_funcs = extract_all(manifest)
    implemented = {k: v for k, v in all_funcs.items() if v == "implemented"}
    stubs = {k for k, v in all_funcs.items() if v == "stub"}
    trusted = {k for k in implemented if tested[k]["passed"] > 0 and tested[k]["failed"] == 0}
    untrusted = [k for k in implemented if k not in trusted]

    trust_pct = len(trusted) / max(len(implemented), 1) * 100
    mod_data = defaultdict(lambda: {"impl": 0, "trusted": 0})
    for k in implemented: mod_data[k.split(".")[0]]["impl"] += 1
    for k in trusted: mod_data[k.split(".")[0]]["trusted"] += 1

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
        for k in sorted(untrusted)[:20]:
            print(f"    - {k}")

    if untracked:
        print(f"\n  🔍 UNTRACKED ({len(untracked)} tests not in coverage_map.json):")
        for t in sorted(set(untracked))[:15]:
            parts = t.split("::")
            name = f"{parts[-2]}::{parts[-1]}" if len(parts) >= 3 else (parts[-1] if parts else t)
            print(f"    - {name}")

    total_tests = len(tests.get("tests", []))
    passed = sum(1 for t in tests.get("tests", []) if t["outcome"] == "passed")
    print(f"\n  ✅ TRUSTED: {len(trusted)}  ⚠️ UNTESTED: {len(untrusted)}  ⬜ STUBS: {len(stubs)}  🔍 UNTRACKED: {len(untracked)}")
    print(f"  📊 {total_tests} tests, {passed} passed\n")

if __name__ == "__main__":
    main()
