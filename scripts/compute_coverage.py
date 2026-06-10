#!/usr/bin/env python3
"""Compute coverage from manifest.yaml + pytest json report."""
import json
import sys
import yaml
from pathlib import Path
from collections import defaultdict

WEIGHTS = {
    "signature": 0.10, "params": 0.20, "modes": 0.35,
    "edges": 0.15, "formats": 0.10, "parity": 0.10,
}


def load_manifest(path: str) -> dict:
    with open(path) as f:
        return yaml.safe_load(f)


def load_test_results(path: str) -> dict:
    with open(path) as f:
        return json.load(f)


def extract_covered_cells(tests: dict) -> dict:
    covered = defaultdict(lambda: {
        "signature_tested": False, "modes": set(), "variants": set(),
        "edges": set(), "formats": set(), "parity_tests": 0, "parity_passes": 0,
    })
    for test in tests.get("tests", []):
        markers = [m.get("name", "") if isinstance(m, dict) else str(m)
                   for m in test.get("markers", [])]
        func_name = mode = variant = edge = fmt = None
        for marker in markers:
            if marker.startswith("covers("):
                parts = [p.strip() for p in marker[7:-1].split(",")]
                func_name = parts[0].strip('"').strip("'")
                for p in parts[1:]:
                    if "=" in p:
                        k, v = p.split("=", 1)
                        k, v = k.strip(), v.strip().strip('"').strip("'")
                        if k == "mode": mode = v
                        elif k == "variant": variant = v
                        elif k == "edge_case": edge = v
                        elif k == "format": fmt = v
        if func_name is None:
            continue
        cell = covered[func_name]
        cell["signature_tested"] = True
        if mode: cell["modes"].add(mode)
        if variant: cell["variants"].add(variant)
        if edge: cell["edges"].add(edge)
        if fmt: cell["formats"].add(fmt)
        outcome = test.get("outcome", "failed")
        if outcome == "passed":
            cell["parity_tests"] += 1
            cell["parity_passes"] += 1
        elif outcome in ("failed", "error"):
            cell["parity_tests"] += 1
    return dict(covered)


def compute_function_coverage(func_def: dict, cells: dict, func_key: str) -> dict:
    cell = cells.get(func_key, {})
    sig_score = 1.0 if cell.get("signature_tested") else 0.0
    expected_variants = func_def.get("param_variants", [])
    expected_modes = set(func_def.get("supported_modes", []))
    expected_edges = func_def.get("edge_cases", [])
    expected_formats = func_def.get("supported_formats", [])

    tested_variants = cell.get("variants", set())
    n_exp_var = max(len(expected_variants), 1)
    n_tested_var = min(len(tested_variants), n_exp_var) if expected_variants else 0
    param_score = n_tested_var / max(n_exp_var, 1)

    total_cells = max(len(expected_modes) * max(n_exp_var, 1), 1)
    covered_cells = len(cell.get("modes", set()) & expected_modes) * max(n_tested_var, 1)
    mode_score = min(covered_cells / total_cells, 1.0)

    edge_score = len(cell.get("edges", set()) & set(expected_edges)) / max(len(expected_edges), 1)
    fmt_score = len(cell.get("formats", set()) & set(expected_formats)) / max(len(expected_formats), 1) if expected_formats else 1.0

    parity_total = cell.get("parity_tests", 0)
    parity_passes = cell.get("parity_passes", 0)
    parity_score = parity_passes / max(parity_total, 1)

    total = (WEIGHTS["signature"] * sig_score + WEIGHTS["params"] * param_score
             + WEIGHTS["modes"] * mode_score + WEIGHTS["edges"] * edge_score
             + WEIGHTS["formats"] * fmt_score + WEIGHTS["parity"] * parity_score)
    return {
        "function": func_key, "signature_score": sig_score,
        "param_score": round(param_score, 3), "mode_score": round(mode_score, 3),
        "edge_score": round(edge_score, 3), "format_score": round(fmt_score, 3),
        "parity_score": round(parity_score, 3), "total": round(total, 3),
        "mode_coverage": f"{covered_cells}/{total_cells}",
        "variant_coverage": f"{n_tested_var}/{n_exp_var}",
    }


def extract_all_functions(manifest: dict) -> dict:
    funcs = {}
    for module_name, module_def in manifest.get("modules", {}).items():
        for method in module_def.get("class_methods", []):
            funcs[f"{module_name}.{method['name']}"] = method
        for method in module_def.get("methods", []):
            funcs[f"{module_name}.{method['name']}"] = method
        for func in module_def.get("functions", []):
            funcs[f"{module_name}.{func['name']}"] = func
    return funcs


def main():
    manifest_path = sys.argv[1] if len(sys.argv) > 1 else "manifest.yaml"
    report_path = sys.argv[2] if len(sys.argv) > 2 else "report.json"
    manifest = load_manifest(manifest_path)
    tests = load_test_results(report_path) if Path(report_path).exists() else {"tests": []}
    cells = extract_covered_cells(tests)
    funcs = extract_all_functions(manifest)
    results = [compute_function_coverage(func_def, cells, key)
               for key, func_def in sorted(funcs.items())]

    module_scores = defaultdict(list)
    for r in results:
        mod = r["function"].split(".")[0] if "." in r["function"] else "unknown"
        module_scores[mod].append(r["total"])

    modules = {mod: {"function_count": len(scores), "average": round(sum(scores)/len(scores), 3)}
               for mod, scores in sorted(module_scores.items())}
    overall = round(sum(r["total"] for r in results) / max(len(results), 1), 3)

    report = {"version": manifest.get("version", "unknown"),
              "pillow_version": manifest.get("pillow_version", "unknown"),
              "overall_coverage": overall, "modules": modules, "functions": results}
    Path("coverage").mkdir(exist_ok=True)
    with open("coverage/report.json", "w") as f:
        json.dump(report, f, indent=2)

    print(f"\n{'='*60}")
    print(f"  pillow-rs Coverage  |  Overall: {overall*100:.1f}%")
    print(f"{'='*60}")
    for mod, info in sorted(modules.items()):
        print(f"  {mod:<25} {info['function_count']:>3} funcs  {info['average']*100:>5.1f}%")
    print(f"{'='*60}\n")


if __name__ == "__main__":
    main()
