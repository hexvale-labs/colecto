import importlib.util
from pathlib import Path
import unittest
import subprocess
import sys
import tempfile
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('resource_bench', Path(__file__).with_name('resource_bench.py'))
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class ResourceBenchTests(unittest.TestCase):
    def test_summary_retains_raw_samples_and_computes_mean_median(self):
        samples = [{'daemon_cpu_seconds': 1, 'daemon_retained_rss_bytes': 10},
                   {'daemon_cpu_seconds': 9, 'daemon_retained_rss_bytes': 30},
                   {'daemon_cpu_seconds': 2, 'daemon_retained_rss_bytes': 20}]
        summary = bench.summarize(samples)
        self.assertEqual(summary['daemon_cpu_seconds'], {'mean': 4, 'median': 2})
        self.assertEqual(summary['daemon_retained_rss_bytes'], {'mean': 20, 'median': 20})
        self.assertEqual(samples[1]['daemon_cpu_seconds'], 9)

    def test_comparison_skips_other_machines_and_workloads(self):
        current = {'compatibility': {'machine': 'one', 'workload': 'v1'},
                   'summary': {'daemon_cpu_seconds': {'mean': 2, 'median': 2}}}
        incompatible = {**current, 'compatibility': {'machine': 'two', 'workload': 'v1'}}
        self.assertIsNone(bench.compare(current, incompatible))
        previous = {**current, 'summary': {'daemon_cpu_seconds': {'mean': 1, 'median': 1}}}
        self.assertEqual(bench.compare(current, previous)['daemon_cpu_seconds']['median_percent'], 100)
        zero = {**current, 'summary': {'daemon_cpu_seconds': {'mean': 0, 'median': 0}}}
        self.assertIsNone(bench.compare(current, zero)['daemon_cpu_seconds']['median_percent'])

    def test_build_and_resource_changes_prevent_comparison(self):
        compatibility = {'build_settings': {'rustc': 'one', 'environment': {}},
                         'resource_limits': {'cgroup_version': 2, 'cpu_affinity': [0],
                                             'cgroup_limits': [{'memory.max': '100'}]}}
        current = {'compatibility': compatibility, 'summary': {}}
        self.assertEqual(bench.compare(current, current), {})
        for key in ('build_settings', 'resource_limits'):
            previous = {**current, 'compatibility': {**compatibility, key: {}}}
            self.assertIsNone(bench.compare(current, previous))
        unknown = {**current, 'compatibility': {**compatibility, 'resource_limits': {
            'cgroup_version': 'unknown', 'cgroup_limits': []}}}
        self.assertIsNone(bench.compare(unknown, unknown))

    def test_resource_limits_include_constraining_ancestors(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'parent/leaf').mkdir(parents=True)
            (root / 'parent/leaf/memory.max').write_text('max')
            (root / 'parent/memory.max').write_text('6000000000')
            (root / 'cpu.max').write_text('200000 100000')
            membership = root / 'membership'
            membership.write_text('0::/parent/leaf\n')
            with patch.object(bench.os, 'sched_getaffinity', return_value={2, 1}):
                result = bench.resource_limits(root, membership)
            self.assertEqual(result['cpu_affinity'], [1, 2])
            self.assertEqual(result['cgroup_limits'], [{'memory.max': 'max'},
                {'memory.max': '6000000000'}, {'cpu.max': '200000 100000'}])

    def test_environment_cannot_connect_real_accounts_or_existing_data(self):
        env = bench.isolated_environment({'PATH': '/bin', 'COLECTO_MAIL_PASSWORD': 'secret',
            'COLECTO_JMAP_TOKEN': 'secret', 'COLECTO_DATA_DIR': '/important', 'COLECTO_PORT': '9999'}, '/tmp/new', 1234)
        self.assertEqual(env['PATH'], '/bin')
        self.assertNotIn('COLECTO_MAIL_PASSWORD', env)
        self.assertNotIn('COLECTO_JMAP_TOKEN', env)
        self.assertEqual(env['COLECTO_DATA_DIR'], '/tmp/new')
        self.assertEqual(env['COLECTO_PORT'], '1234')
        self.assertEqual(env['COLECTO_UPDATE_CHECK'], 'off')

    def test_stop_process_terminates_its_isolated_process_group(self):
        process = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(60)'], start_new_session=True)
        try:
            bench.stop_process(process)
            self.assertIsNotNone(process.poll())
            bench.stop_process(process)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()

    def test_live_process_memory_is_not_high_water_mark(self):
        result = bench.process_sample(__import__('os').getpid())
        self.assertGreater(result['rss_bytes'], 0)
        self.assertGreaterEqual(result['peak_rss_bytes'], result['rss_bytes'])
        self.assertGreaterEqual(result['cpu_seconds'], 0)


if __name__ == '__main__':
    unittest.main()
