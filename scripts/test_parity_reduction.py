"""Safety checks for measured test removal, independent of the real corpus."""
from copy import deepcopy
import unittest
from unittest.mock import patch

from build_migration_parity_inputs import case_signature, merge_workflow_aliases
from reduce_migration_parity_cases import bisect_removals, compare, require_gpu_dispatch


class ReductionTests(unittest.TestCase):
    def test_equal_totals_cannot_hide_replaced_locations(self):
        before = ({'lines': {1, 2, 3}}, {'lines': {1, 2}})
        after = ({'lines': {1, 2, 3}}, {'lines': {1, 3}})
        result = compare(before, after)
        self.assertFalse(result['equal'])
        self.assertEqual(result['dimensions']['lines']['lost'], 1)
        self.assertEqual(result['dimensions']['lines']['gained'], 1)

    def test_losing_an_uncovered_location_is_an_inventory_change(self):
        result = compare(({'regions': {1, 2}}, {'regions': {1}}),
                         ({'regions': {1}}, {'regions': {1}}))
        self.assertFalse(result['equal'])
        self.assertTrue(result['dimensions']['regions']['inventory_changed'])

    def test_identical_location_sets_pass(self):
        value = ({'branches': {(4, True), (4, False)}}, {'branches': {(4, True)}})
        self.assertTrue(compare(value, value)['equal'])

    def test_restore_half_preserves_independent_witnesses_in_both_halves(self):
        removed, attempts = [], []
        needed = {23, 78}

        def accept(batch):
            attempts.append(list(batch))
            if needed.intersection(removed + batch):
                return False
            removed.extend(batch)
            return True

        accepted, retained = bisect_removals(list(range(100)), accept)
        self.assertEqual([len(x) for x in attempts[:2]], [100, 50])
        self.assertEqual(set(retained), needed)
        self.assertEqual(set(accepted), set(range(100)) - needed)
        self.assertEqual(accepted, removed)

    def test_cumulative_removals_keep_one_of_interchangeable_witnesses(self):
        removed = []

        def accept(batch):
            if {2, 3} <= set(removed + batch):
                return False
            removed.extend(batch)
            return True

        accepted, retained = bisect_removals([0, 1, 2, 3], accept)
        self.assertEqual(accepted, [0, 1, 2])
        self.assertEqual(retained, [3])

    def test_failed_measurement_aborts_instead_of_accepting_or_bisecting(self):
        def broken(batch):
            raise RuntimeError('collector failed')

        with self.assertRaisesRegex(RuntimeError, 'collector failed'):
            bisect_removals(list(range(100)), broken)

    def test_gpu_initialization_or_cpu_fallback_cannot_count_as_hardware_execution(self):
        path = 'pillow-rs/src/compute/pool_gpu/mod.rs'
        with patch('pathlib.Path.read_text', return_value='initialize();\npass.dispatch_workgroups(1, 1, 1);\n'):
            with self.assertRaisesRegex(ValueError, 'hardware dispatch'):
                require_gpu_dispatch({'rust_lines': {(path, 1)}})
            require_gpu_dispatch({'rust_lines': {(path, 1), (path, 2)}})


class WorkflowIdentityTests(unittest.TestCase):
    @staticmethod
    def workflow(case_id, load_id, call_id, requirement):
        return {
            'case_id': case_id, 'covers': [requirement],
            'surface': 'PIL.Image.Image', 'operation': 'getexif',
            'target_profiles': ['python-cpu'], 'assets': [],
            'steps': [
                {'step_id': load_id, 'surface': 'PIL.Image', 'operation': 'new',
                 'arguments': {'data': {'kind': 'literal', 'value': {'step_id': 'load'}}}},
                {'step_id': call_id, 'surface': 'PIL.Image.Image', 'operation': 'getexif',
                 'receiver': {'kind': 'binding', 'step_id': load_id}, 'arguments': {}},
            ],
            'observations': [call_id],
            'comparisons': {call_id: {'method': 'exact'}},
        }

    def test_step_aliases_merge_requirements_and_preserve_first_stable_id(self):
        first = self.workflow('original', 'load', 'call', 'behavior')
        alias = self.workflow('alias', 'renamed-load', 'renamed-call', 'performance')
        self.assertEqual(case_signature(first), case_signature(alias))
        unique, aliases = merge_workflow_aliases([first, alias])
        self.assertEqual(len(unique), 1)
        self.assertEqual(unique[0]['case_id'], 'original')
        self.assertEqual(unique[0]['covers'], ['behavior', 'performance'])
        self.assertEqual(aliases, {'alias': 'original'})

    def test_literals_comparison_policies_profiles_and_observations_remain_distinct(self):
        first = self.workflow('original', 'load', 'call', 'behavior')
        alias = self.workflow('alias', 'renamed-load', 'renamed-call', 'performance')
        changed_literal = deepcopy(alias)
        changed_literal['steps'][0]['arguments']['data']['value']['step_id'] = 'renamed-load'
        changed_comparison = deepcopy(alias)
        changed_comparison['comparisons']['renamed-call'] = {'method': 'tolerance', 'value': 1}
        changed_profile = deepcopy(alias)
        changed_profile['target_profiles'] = ['javascript-core-wasm']
        changed_observation = deepcopy(alias)
        changed_observation['observations'] = ['renamed-load', 'renamed-call']
        for changed in (changed_literal, changed_comparison, changed_profile, changed_observation):
            with self.subTest(changed=changed):
                self.assertNotEqual(case_signature(first), case_signature(changed))
                self.assertEqual(len(merge_workflow_aliases([deepcopy(first), changed])[0]), 2)


if __name__ == '__main__':
    unittest.main()
