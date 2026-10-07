"""Protect comparison meaning as the public benchmark UI changes."""
import copy
import unittest

from docs_benchmark_view import compare, describe, duration, facets, render_dashboard, speed_label, workload_distributions


def row(subject="fontdone", median=10, **changes):
    result = dict(workload="dejavusans_load_20", subject=subject, median_us=median,
                  p90_us=12, p95_us=None, sample_count=10, sample_unit="benchmark sample",
                  status="completed", policy={"boundary":"public operation"}, context={},
                  correctness="timing_only", requested_backend="native", actual_backend="native",
                  terminal_complete=None, fallback_reasons={})
    result.update(changes)
    return result


class BenchmarkViewTests(unittest.TestCase):
    def test_factor_direction_and_absolute_units(self):
        self.assertEqual(compare(row(median=5),row("FreeType",20))[0],4)
        self.assertEqual(speed_label(4),'4× faster')
        self.assertEqual(speed_label(.25),'4× slower')
        self.assertEqual(speed_label(1),'Same median time')
        self.assertEqual(duration(.007),'7 ns')
        self.assertEqual(duration(2500),'2.5 ms')
        self.assertEqual(duration(None),'Not measured')
        self.assertIn('less time',speed_label(1.001))

    def test_missing_zero_and_failed_values_are_not_winners(self):
        baseline=row('FreeType',20)
        for modified in [row(median=None),row(median=0),row(status='failed'),
                         row(correctness='source_target_match: fail'),
                         row(correctness='output hash match: False; byte length match: True')]:
            with self.subTest(modified=modified):
                self.assertIsNone(compare(modified,baseline)[0])
        self.assertIsNone(compare(row(),None)[0])
        self.assertIsNone(compare(row(),row('FreeType',0))[0])

    def test_same_workload_and_measurement_conditions_required(self):
        baseline=row('FreeType',20)
        for changes in [dict(workload='other'),dict(policy={'boundary':'whole process'}),
                        dict(context={'size':64}),dict(sample_unit='round median'),dict(sample_count=5)]:
            with self.subTest(changes=changes):
                self.assertIsNone(compare(row(**changes),baseline)[0])

    def test_cross_runner_rows_never_form_a_speed_ratio(self):
        baseline = row("pillow", 20, machine_id="ubuntu-x64")
        target = row("python-simd", 10, machine_id="macos-arm64", requested_backend="simd", actual_backend="simd")
        ratio, state, reason = compare(target, baseline)
        self.assertIsNone(ratio)
        self.assertEqual(state, "unavailable")
        self.assertEqual(reason, "Different runner cohorts")

    def test_overall_boxplot_uses_paired_exact_output_workloads_per_runner(self):
        machines = {
            "linux": {"id": "linux", "label": "Ubuntu 24.04 · x86_64"},
            "mac": {"id": "mac", "label": "macOS 15 · arm64"},
        }
        rows = []
        for machine_id, times in (("linux", (20, 10)), ("mac", (30, 10))):
            for index, (pillow_time, target_time) in enumerate((times, (40, 20))):
                workload = f"pipeline-op.test-{index}"
                rows.extend([
                    row("pillow", pillow_time, workload=workload, machine_id=machine_id,
                        correctness="parity_pass: pass", requested_backend="pillow", actual_backend="pillow"),
                    row("python-simd", target_time, workload=workload, machine_id=machine_id,
                        correctness="parity_pass: pass", requested_backend="simd", actual_backend="simd"),
                ])
        rows.extend([
            row("pillow", 10, workload="pipeline-op.timing-only", machine_id="linux",
                correctness="successful_execution: pass", requested_backend="pillow", actual_backend="pillow"),
            row("python-simd", 1, workload="pipeline-op.timing-only", machine_id="linux",
                correctness="successful_execution: pass", requested_backend="simd", actual_backend="simd"),
        ])
        grouped = {}
        for item in rows:
            grouped.setdefault((item["machine_id"], item["workload"]), []).append(item)
        distributions = workload_distributions(grouped, machines, ["python-simd"], "pillow")
        self.assertEqual(len(distributions), 2)
        self.assertEqual({item["machine_id"] for item in distributions}, {"linux", "mac"})
        by_machine = {item["machine_id"]: item for item in distributions}
        self.assertEqual(by_machine["linux"]["ratios"], [2, 2])
        self.assertEqual(by_machine["mac"]["ratios"], [2, 3])
        self.assertEqual(by_machine["linux"]["geomean"], 2)
        snapshot = dict(rows=rows, machines=list(machines.values()), environment={"os": "GitHub runners"}, measured_at="2026-10-07")
        text = render_dashboard(snapshot, dict(benchmark={"kind": "pillow"}))
        self.assertIn("Overall speedup by workload", text)
        self.assertLess(text.index('<section class="bench-overall">'), text.index('<div class="bench-summary">'))
        self.assertIn('data-machine="linux"', text)
        self.assertIn('data-machine="mac"', text)
        self.assertIn('id="bench-machine"', text)
        self.assertIn('id="bench-group"', text)
        self.assertIn('id="bench-mode"', text)
        self.assertIn('id="bench-subject"', text)
        self.assertIn('data-sort="runner"', text)
        self.assertIn("geomean 2× faster", text)
        self.assertIn('data-ratio-primary="2.0"', text)
        self.assertIn('data-ratio-primary=""', text)
        self.assertIn('<b data-score="faster">4</b> faster', text)

    def test_boxplot_includes_pillow_simd_pair_against_its_matched_host(self):
        machine = "linux-pillow-simd"
        checked = "parity_pass: pass"
        rows = [
            row("pillow", 20, workload="pipeline-op.blur", machine_id=machine, correctness=checked,
                requested_backend="pillow", actual_backend="pillow"),
            row("pillow-simd", 10, workload="pipeline-op.blur", machine_id=machine, correctness=checked,
                requested_backend="pillow-simd", actual_backend="pillow-simd"),
            row("python-simd", 5, workload="pipeline-op.blur", machine_id=machine, correctness=checked,
                requested_backend="simd", actual_backend="simd"),
        ]
        machines = {machine: {"id": machine, "label": "Ubuntu 24.04 · x86_64 · Pillow-SIMD paired cohort"}}
        grouped = {(machine, "pipeline-op.blur"): rows}
        distributions = workload_distributions(grouped, machines, ["python-simd", "pillow-simd"], "pillow")
        pairs = {(item["subject"], item["baseline"], tuple(item["ratios"])) for item in distributions}
        self.assertIn(("python-simd", "pillow", (4,)), pairs)
        self.assertIn(("python-simd", "pillow-simd", (2,)), pairs)
        self.assertIn(("pillow-simd", "pillow", (2,)), pairs)

    def test_overall_distribution_summarizes_operations_and_pipelines_separately(self):
        machine = "ubuntu-x64"
        machines = {machine: {"id": machine, "label": "Ubuntu 24.04 · x86_64"}}
        rows = []
        for workload, pillow_time, target_time in (
            ("pipeline-op.blur.material", 20, 10),
            ("pipeline.quick.gray", 40, 10),
        ):
            context = {"mode": "L", "operation_class": "filter"}
            rows.extend([
                row("pillow", pillow_time, workload=workload, machine_id=machine,
                    correctness="parity_pass: pass", requested_backend="pillow", actual_backend="pillow",
                    context=context),
                row("python-cpu", target_time, workload=workload, machine_id=machine,
                    correctness="parity_pass: pass", requested_backend="cpu", actual_backend="cpu",
                    context=context),
            ])
        grouped = {}
        for item in rows:
            grouped.setdefault((item["machine_id"], item["workload"]), []).append(item)
        distributions = workload_distributions(grouped, machines, ["python-cpu"], "pillow")
        self.assertEqual({item["scope"] for item in distributions}, {"operations", "pipelines"})
        by_scope = {item["scope"]: item for item in distributions}
        self.assertEqual(by_scope["operations"]["geomean"], 2)
        self.assertEqual(by_scope["pipelines"]["geomean"], 4)
        dashboard = render_dashboard({"rows": rows, "machines": list(machines.values()),
                                      "environment": {}, "measured_at": "2026-10-07"},
                                     {"benchmark": {"kind": "pillow"}})
        self.assertIn("individual operations", dashboard)
        self.assertIn("pipeline workflows", dashboard)

    def test_timing_only_is_not_promoted_to_checked_output(self):
        baseline=row('FreeType',20)
        self.assertEqual(compare(row(),baseline)[1],'timing')
        match='output hash match: True; byte length match: True'
        self.assertEqual(compare(row(correctness=match),baseline)[1],'timing')
        self.assertEqual(compare(row(correctness=match),row('FreeType',20,correctness=match))[1],'checked')
        self.assertIsNone(compare(row(correctness='new_unknown_gate'),baseline)[0])

    def test_gpu_requires_completion_and_fallback_is_named(self):
        baseline=row('pillow',20)
        target=row('python-gpu',10,requested_backend='gpu',actual_backend='cpu',terminal_complete=False)
        self.assertIsNone(compare(target,baseline)[0])
        target['terminal_complete']=True
        ratio, state, note = compare(target,baseline)
        self.assertIsNone(ratio)
        self.assertEqual(state, 'unavailable')
        self.assertIn('fallback', note)
        text=render_dashboard(dict(rows=[baseline,target],environment={'os':'Test'},measured_at='2026-09-16'),
                              dict(project='pillow-rs',benchmark={'kind':'pillow'}))
        self.assertIn('Actual: cpu',text)
        self.assertIn('actual: cpu',text)

    def test_cpu_and_simd_fallbacks_are_not_compared_as_native_performance(self):
        baseline = row("pillow", 20)
        for backend, actual, fallbacks in (
            ("simd", "cpu", {}),
            ("simd", "simd", {"unsupported_operation": 1}),
            ("cpu", "unknown", {}),
        ):
            target = row(
                f"python-{backend}", 10, requested_backend=backend,
                actual_backend=actual, fallback_reasons=fallbacks,
            )
            with self.subTest(backend=backend, actual=actual, fallbacks=fallbacks):
                self.assertIsNone(compare(target, baseline)[0])

    def test_parallel_cpu_uses_ordinary_pillow_baseline(self):
        target = row("python-parallel-cpu", 10, workload="pipeline-op.resize")
        default_pillow = row("pillow", 40, workload="pipeline-op.resize")
        self.assertEqual(compare(target, default_pillow)[0], 4)
        snapshot = dict(rows=[default_pillow, target], environment={"os": "Test"}, measured_at="2026-09-16")
        text = render_dashboard(snapshot, dict(project="pillow-rs", benchmark={"kind": "pillow"}))
        self.assertIn("pillow-rs · Parallel CPU", text)
        self.assertIn('data-ratio="4.0"', text)
        self.assertIn('data-baseline-for="pillow"', text)
        self.assertNotIn('data-subject="pillow-parallel-cpu"', text)
        self.assertIn("Parallel CPU is measured separately with the opt-in Rayon feature", text)

    def test_dashboard_renders_separate_pipeline_and_operation_tables_with_mode_and_type(self):
        rows = [
            row("pillow", 20, workload="pipeline-chain.gray", context={"mode": "L", "size": [8, 8], "operation_class": "point"}),
            row("python-cpu", 10, workload="pipeline-chain.gray", context={"mode": "L", "size": [8, 8], "operation_class": "point"}),
            row("pillow", 30, workload="pipeline-op.resize.material", context={"mode": "RGB", "size": [8, 8], "operation_class": "geometry"}),
            row("python-cpu", 15, workload="pipeline-op.resize.material", context={"mode": "RGB", "size": [8, 8], "operation_class": "geometry"}),
        ]
        text = render_dashboard(dict(rows=rows, environment={"os": "Test"}, measured_at="2026-09-16"),
                                dict(project="pillow-rs", benchmark={"kind": "pillow"}))
        self.assertIn("Pipeline benchmarks", text)
        self.assertIn("Individual operation benchmarks", text)
        self.assertIn('data-kind="pipelines"', text)
        self.assertIn('data-kind="operations"', text)
        self.assertIn('id="bench-mode"', text)
        self.assertIn('data-sort="type"', text)
        self.assertIn('data-sort="mode"', text)
        self.assertIn('data-group="geometry"', text)
        self.assertIn('data-mode="RGB"', text)

    def test_current_workload_catalog_maps_all_entries_to_one_of_two_tables(self):
        import json
        from pathlib import Path

        source = Path(__file__).resolve().parents[1] / "pillow-rs/tests/fixtures/inputs/benchmark/pipeline-operations.json"
        workloads = json.loads(source.read_text())["workloads"]
        categories = [facets(row(workload=workload["workload_id"], context=workload.get("context", {})), "pillow")[0]
                     for workload in workloads]
        # Pin the complete generated catalog so additions and removals must
        # keep both public benchmark tables current. These counts match the
        # checked-in generated input catalog.
        # The native L uniform 5x5 benchmark is an additional operation row.
        self.assertEqual(len(workloads), 658)
        self.assertEqual(categories.count("operations"), 196)
        self.assertEqual(categories.count("pipelines"), 462)

    def test_all_observations_retained_without_mutating_snapshot(self):
        snapshot=dict(rows=[row('FreeType',20),row(),row('unknown',None)],environment={'os':'Test'},measured_at='2026-09-16')
        original=copy.deepcopy(snapshot)
        text=render_dashboard(snapshot,dict(project='fontdone',benchmark={'kind':'fontdone'}))
        self.assertEqual(text.count('class="bench-workload"'),1)
        for subject in ['FreeType','fontdone','unknown']:
            self.assertIn(f'data-subject="{subject}"',text)
        self.assertIn('2× faster',text)
        self.assertIn('Not comparable',text)
        self.assertEqual(snapshot,original)

    def test_unverified_and_missing_pairs_are_unavailable_in_summary(self):
        snapshot=dict(rows=[row('FreeType',20),row(),row('FreeType',30,workload='other')],environment={'os':'Test'},measured_at='2026-09-16')
        text=render_dashboard(snapshot,dict(project='fontdone',benchmark={'kind':'fontdone'}))
        self.assertIn('data-score="unavailable">2</span>',text)
        self.assertIn('data-excluded-count>1</span>', text)

    def test_injected_labels_are_escaped(self):
        snapshot=dict(rows=[row(workload='<script>alert(1)</script>',subject='" onclick="alert(1)')],environment={'os':'<host>'},measured_at='2026-09-16')
        text=render_dashboard(snapshot,dict(project='fontdone',benchmark={'kind':'fontdone'}))
        self.assertNotIn('<script>',text)
        self.assertNotIn('data-subject="" onclick=',text)

    def test_operation_groups_and_pipeline_groups_are_distinct_and_searchable(self):
        pipeline = row(workload="pipeline-chain.reviewed.resize-rotate-crop")
        operation = row(
            workload="pipeline-op.resize.materialized",
            context={"mode": "RGB", "size": [1024, 768], "operation_class": "geometry"},
        )
        self.assertEqual(facets(pipeline, "pillow")[0], "pipelines")
        self.assertEqual(facets(operation, "pillow")[:3], ("operations", "geometry", "RGB"))

    def test_no_extreme_ratio_becomes_infinity_or_zero(self):
        self.assertIsNone(compare(row(median=1e-310),row('FreeType',1e300))[0])
        self.assertIsNone(compare(row(median=1e300),row('FreeType',1e-310))[0])
