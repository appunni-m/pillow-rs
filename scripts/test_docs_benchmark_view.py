"""Protect comparison meaning as the public benchmark UI changes."""
import copy
import unittest

from docs_benchmark_view import compare, describe, duration, render_dashboard, speed_label


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
        self.assertEqual(compare(target,baseline)[0],2)
        text=render_dashboard(dict(rows=[baseline,target],environment={'os':'Test'},measured_at='2026-09-16'),
                              dict(project='pillow-rs',benchmark={'kind':'pillow'}))
        self.assertIn('Actual: cpu',text)
        self.assertIn('actual: cpu',text)

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

    def test_missing_target_cells_count_as_unavailable(self):
        snapshot=dict(rows=[row('FreeType',20),row(),row('FreeType',30,workload='other')],environment={'os':'Test'},measured_at='2026-09-16')
        text=render_dashboard(snapshot,dict(project='fontdone',benchmark={'kind':'fontdone'}))
        self.assertIn('data-score="unavailable">1</span>',text)

    def test_injected_labels_are_escaped(self):
        snapshot=dict(rows=[row(workload='<script>alert(1)</script>',subject='" onclick="alert(1)')],environment={'os':'<host>'},measured_at='2026-09-16')
        text=render_dashboard(snapshot,dict(project='fontdone',benchmark={'kind':'fontdone'}))
        self.assertNotIn('<script>',text)
        self.assertNotIn('data-subject="" onclick=',text)

    def test_pipelines_are_separate_from_individual_operations(self):
        self.assertEqual(describe(row(workload='pipeline-chain.reviewed.resize-rotate-crop'),'pillow')[1],'Pipelines')
        self.assertEqual(describe(row(workload='pipeline-op.resize.benchmark-materialized'),'pillow')[1],'Operations')
        self.assertEqual(describe(row(workload='pipeline-lifecycle.new'),'pillow')[1],'Lifecycle')

    def test_no_extreme_ratio_becomes_infinity_or_zero(self):
        self.assertIsNone(compare(row(median=1e-310),row('FreeType',1e300))[0])
        self.assertIsNone(compare(row(median=1e300),row('FreeType',1e-310))[0])
