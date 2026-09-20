#!/usr/bin/env python3
"""Measure removal batches against fixed coverage; never edit the active corpus.

Candidates are reviewed input pairs, not a claim that coverage proves equivalent
behavior. Every accepted removal preserves the baseline's exact covered and
instrumented locations separately for CPU, SIMD and GPU and both host languages.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
from functools import cache
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from typing import Callable

ROOT = Path(__file__).resolve().parents[1]


def read(path: Path):
    return json.loads(path.read_text())


def write(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


@cache
def measured_path(raw: str) -> str | None:
    path = Path(raw)
    if path.is_absolute():
        try:
            path = path.relative_to(ROOT)
        except ValueError:
            return None
    value = path.as_posix()
    return value if value.startswith(('pillow-rs/src/', 'pillow-rs-py/src/', 'pillow-rs-py/python/')) else None


def locations(directory: Path) -> tuple[dict[str, set], dict[str, set]]:
    """Use LLVM region coordinates/branch outcomes and LCOV executable lines.

    Function names retain monomorphization identity. Counts become hit/miss;
    execution frequency is deliberately not confused with coverage. External
    dependency/runtime files are outside this repository's reduction contract.
    """
    inventory, hit = defaultdict(set), defaultdict(set)

    def add(kind, key, covered):
        inventory[kind].add(key)
        if covered:
            hit[kind].add(key)

    path = None
    for line in (directory / 'rust.lcov').read_text().splitlines():
        if line.startswith('SF:'):
            path = measured_path(line[3:])
        elif path and line.startswith('DA:'):
            number, count, *_ = line[3:].split(',')
            add('rust_lines', (path, int(number)), int(count) > 0)
    for data in read(directory / 'rust.json')['data']:
        for function in data['functions']:
            names = function['filenames']
            name = function['name']
            owned = tuple(filter(None, (measured_path(p) for p in names)))
            if owned:
                add('rust_functions', (owned, name), function['count'] > 0)
            for region in function['regions']:
                path = measured_path(names[region[5]])
                if path:
                    key = (path, name, *region[:4], *region[5:])
                    add('rust_regions', key, region[4] > 0)
            for branch in function.get('branches', []):
                path = measured_path(names[branch[6]])
                if path:
                    for outcome in (0, 1):
                        key = (path, name, *branch[:4], *branch[6:], outcome)
                        add('rust_branches', key, branch[4 + outcome] > 0)
    for raw_path, data in read(directory / 'python.json')['files'].items():
        path = measured_path(raw_path)
        if path:
            for covered, field in ((True, 'executed_lines'), (False, 'missing_lines')):
                for number in data[field]:
                    add('python_lines', (path, number), covered)
            for covered, field in ((True, 'executed_branches'), (False, 'missing_branches')):
                for edge in data[field]:
                    add('python_branches', (path, *edge), covered)
    required = {'rust_lines', 'rust_functions', 'rust_regions', 'rust_branches', 'python_lines', 'python_branches'}
    if set(inventory) != required or any(not hit[k] for k in required):
        raise ValueError('incomplete or zero-hit coverage dimensions')
    return dict(inventory), dict(hit)


def compare(baseline, current) -> dict:
    old_inventory, old_hit = baseline
    new_inventory, new_hit = current
    result = {}
    for kind in sorted(old_inventory.keys() | new_inventory.keys()):
        lost = old_hit.get(kind, set()) - new_hit.get(kind, set())
        gained = new_hit.get(kind, set()) - old_hit.get(kind, set())
        inventory_changed = old_inventory.get(kind, set()) != new_inventory.get(kind, set())
        result[kind] = {
            'baseline_covered': len(old_hit.get(kind, set())),
            'current_covered': len(new_hit.get(kind, set())),
            'total': len(old_inventory.get(kind, set())),
            'lost': len(lost), 'gained': len(gained),
            'inventory_changed': inventory_changed,
            'lost_examples': sorted(lost, key=str)[:10],
            'gained_examples': sorted(gained, key=str)[:10],
        }
    return {'equal': all(not (v['lost'] or v['gained'] or v['inventory_changed']) for v in result.values()), 'dimensions': result}


def require_gpu_dispatch(hit: dict[str, set]) -> None:
    """A selected GPU backend can silently fall back when no device is visible."""
    source = 'pillow-rs/src/compute/pool_gpu/mod.rs'
    dispatches = {
        (source, line)
        for line, text in enumerate((ROOT / source).read_text().splitlines(), 1)
        if '.dispatch_workgroups(' in text
    }
    if not dispatches.intersection(hit['rust_lines']):
        raise ValueError('GPU coverage did not reach a hardware dispatch; check adapter access instead of accepting CPU fallback')


def bisect_removals(candidates: list[str], accept: Callable[[list[str]], bool]) -> tuple[list[str], list[str]]:
    """Try the whole batch, then restore half and recursively test both halves.

    The callback evaluates cumulative removals. Both halves must be explored:
    a single numeric binary search would miss independent coverage witnesses.
    """
    if not candidates:
        return [], []
    if accept(candidates):
        return candidates, []
    if len(candidates) == 1:
        return [], candidates
    middle = len(candidates) // 2
    left_removed, left_kept = bisect_removals(candidates[:middle], accept)
    right_removed, right_kept = bisect_removals(candidates[middle:], accept)
    return left_removed + right_removed, left_kept + right_kept


def verify_receipts(directory: Path, backend: str, excluded: list[str], expected_selected: set[str]) -> dict:
    receipt = read(directory / 'rust.json.context.json')
    if receipt['test_status'] != 'passed':
        raise ValueError(f'failed coverage receipt: {directory}')
    for report in ('rust.json', 'rust.lcov', 'python.json'):
        context = read(directory / (report + '.context.json'))
        if context['report_sha256'] != digest(directory / report):
            raise ValueError(f'coverage digest mismatch: {directory / report}')
        for field in ('source_hashes', 'input_hashes', 'build_id', 'execution'):
            if context[field] != receipt[field]:
                raise ValueError(f'incompatible {field}: {report}')
    for key, value in receipt['source_hashes'].items():
        if digest(ROOT / key) != value:
            raise ValueError(f'source changed since measurement: {key}')
    for key, value in receipt['input_hashes'].items():
        path = ROOT / key
        if (digest(path) if path.is_file() else None) != value:
            raise ValueError(f'input changed since measurement: {key}')
    executions = receipt['execution']['backends']
    if len(executions) != 1 or executions[0]['backend'] != backend:
        raise ValueError('backend receipt mismatch')
    result = read(directory / 'coverage.json')
    selected = {case_id for plan in result['plans'] for case_id in plan['selected']['parity_case_ids']}
    if selected != expected_selected - set(excluded):
        raise ValueError('coverage did not execute the exact requested case selection')
    if any(plan['selected']['command_ids'] for plan in result['plans']):
        raise ValueError('native supplements changed the canonical parity scope')
    return receipt


def measure(directory: Path, backend: str, excluded: list[str], selected: set[str]) -> tuple[dict, tuple]:
    directory.mkdir(parents=True, exist_ok=True)
    if not (directory / 'complete.json').exists():
        command = [
            'make', 'migration-parity-coverage-rust', f'PYTHON={sys.executable}',
            f'MIGRATION_TARGET_BACKEND={backend}', 'MIGRATION_COVERAGE_PARITY_ONLY=1',
            f'MIGRATION_COVERAGE_EXCLUDE_CASE_IDS={" ".join(excluded)}',
            f'MIGRATION_RUST_COVERAGE_OUTPUT={directory / "coverage.json"}',
            f'MIGRATION_RUST_COVERAGE_LLVM_REPORT={directory / "rust.json"}',
            f'MIGRATION_RUST_COVERAGE_LCOV_REPORT={directory / "rust.lcov"}',
            f'MIGRATION_RUST_COVERAGE_PYTHON_REPORT={directory / "python.json"}',
        ]
        write(directory / 'request.json', {'backend': backend, 'excluded': excluded, 'argv': command})
        print(f'collecting {backend}: {len(selected) - len(excluded)} cases; {directory}', flush=True)
        with (directory / 'run.log').open('w') as log:
            subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True,
                           env={**os.environ, 'PYTHONUNBUFFERED': '1'})
        verify_receipts(directory, backend, excluded, selected)
        write(directory / 'complete.json', {'backend': backend, 'excluded': excluded})
    if read(directory / 'complete.json') != {'backend': backend, 'excluded': excluded}:
        raise ValueError('existing measurement used a different selection')
    receipt = verify_receipts(directory, backend, excluded, selected)
    measured = locations(directory)
    if backend == 'gpu':
        require_gpu_dispatch(measured[1])
    return receipt, measured


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidates', type=Path, required=True, help='Reviewed JSON array of case_id/replacement_case_id objects')
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--batch-size', type=int, default=100)
    args = parser.parse_args()
    if not 1 <= args.batch_size <= 100:
        parser.error('batch size must be between 1 and 100')
    from run_migration_parity import load_cases, load_manifest
    from run_migration_coverage import load_coverage_plans, scope_coverage_plans, coverage_not_applicable_operations

    manifest = load_manifest(ROOT / 'pillow-rs/tests/fixtures/manifest.yaml')
    cases, _ = load_cases(manifest, case_ids=None, surface=None)
    by_id = {case['case_id']: case for case in cases}
    plans, _ = load_coverage_plans(manifest)
    _, selected = scope_coverage_plans(plans, by_id, excluded_operations=coverage_not_applicable_operations(manifest))
    candidates = read(args.candidates)
    ids = [c['case_id'] for c in candidates]
    if len(ids) != len(set(ids)) or not set(ids) <= selected:
        raise ValueError('candidates must be unique active coverage case IDs')
    for candidate in candidates:
        replacement = candidate['replacement_case_id']
        if replacement not in selected or replacement in ids:
            raise ValueError('replacement must remain in every trial')
        before, after = by_id[candidate['case_id']], by_id[replacement]
        if before['target_profiles'] != after['target_profiles']:
            raise ValueError(f'replacement must preserve target profiles: {candidate["case_id"]}')
    required = {(r, p) for cid in selected for r in by_id[cid]['covers'] for p in by_id[cid]['target_profiles']}
    retained = {(r, p) for cid in selected - set(ids) for r in by_id[cid]['covers'] for p in by_id[cid]['target_profiles']}
    if required != retained:
        raise ValueError('candidate removals would drop a requirement/profile mapping from the retained corpus')
    output = args.output_dir.resolve()
    request = {'candidates_sha256': digest(args.candidates), 'batch_size': args.batch_size, 'backends': ['cpu', 'simd', 'gpu']}
    request_path = output / 'request.json'
    if request_path.exists() and read(request_path) != request:
        raise ValueError('output directory belongs to a different reduction request')
    write(request_path, request)
    baselines = {b: measure(output / 'baseline' / b, b, [], selected) for b in request['backends']}
    removed, kept, trials = [], [], []
    counter = 0

    def accept(batch):
        nonlocal counter
        counter += 1
        exclusions = sorted(removed + batch)
        trial = {'trial': counter, 'batch': batch, 'removed_total': len(exclusions), 'backends': {}}
        for backend in request['backends']:
            receipt, current = measure(output / f'trial-{counter:04d}' / backend, backend, exclusions, selected)
            baseline_receipt, baseline = baselines[backend]
            for field in ('source_hashes', 'input_hashes', 'build_id'):
                if receipt[field] != baseline_receipt[field]:
                    raise ValueError(f'baseline {field} changed')
            comparison = compare(baseline, current)
            trial['backends'][backend] = comparison
            if not comparison['equal']:
                break
        trial['accepted'] = len(trial['backends']) == 3 and all(c['equal'] for c in trial['backends'].values())
        if trial['accepted']:
            removed.extend(batch)
        trials.append(trial)
        write(output / 'progress.json', {'removed': removed, 'kept': kept, 'trials': trials})
        print(f'trial {counter}: {"remove" if trial["accepted"] else "restore and bisect"} {len(batch)}; accepted total {len(removed)}', flush=True)
        return trial['accepted']

    for offset in range(0, len(ids), args.batch_size):
        _, retained = bisect_removals(ids[offset:offset + args.batch_size], accept)
        kept.extend(retained)
    write(output / 'result.json', {'request': request, 'baseline_cases': len(selected), 'removed': removed, 'kept': kept, 'trials': trials})
    print(f'Complete: {len(removed)} removable, {len(kept)} retained; active inputs were not edited.', flush=True)


if __name__ == '__main__':
    main()
