"""Malformed benchmark comparisons must fail before producing paired statistics."""
import csv
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'scripts/summarize_udp_transport.py'
DATA = ROOT / 'docs/benchmarks/data/avatar-udp-2026-11-14/vr2.csv'


class TrialPairingTests(unittest.TestCase):
    def setUp(self):
        self.rows = list(csv.DictReader(line for line in DATA.read_text().splitlines()
                                       if not line.startswith('#')))

    def run_report(self, rows):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'data.csv'
            with path.open('w') as out:
                writer = csv.DictWriter(out, fieldnames=list(self.rows[0]))
                writer.writeheader()
                writer.writerows(rows)
            return subprocess.run([sys.executable, str(SCRIPT), str(path)],
                                  capture_output=True, text=True)

    def test_valid_row_order_and_single_mode_reports(self):
        normal = self.run_report(self.rows)
        reversed_rows = self.run_report(list(reversed(self.rows)))
        self.assertEqual(normal.returncode, 0, normal.stderr)
        self.assertEqual(normal.stdout, reversed_rows.stdout)
        for mode in ['runtime', 'separate', 'shared']:
            report = self.run_report([r for r in self.rows if r['mode'] == mode])
            self.assertEqual(report.returncode, 0, report.stderr)

    def test_duplicates_on_both_sides_and_runtime(self):
        for mode in ['separate', 'shared', 'runtime']:
            row = next(r for r in self.rows if r['mode'] == mode and r['kind'] == 'udp')
            result = self.run_report([*self.rows, dict(row)])
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('duplicate', result.stderr)

    def test_unmatched_trial_ids_in_both_directions(self):
        for mode in ['separate', 'shared']:
            row = dict(next(r for r in self.rows if r['mode'] == mode and r['kind'] == 'udp'))
            row['trial'] = '999'
            row['dropped'] = '1'  # Validate identities before excluding lossy trials.
            result = self.run_report([*self.rows, row])
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('unpaired', result.stderr)

    def test_missing_measurement_kind_is_not_single_mode(self):
        rows = [r for r in self.rows if not (r['mode'] == 'shared' and r['kind'] == 'udp')]
        result = self.run_report(rows)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('unpaired measurement', result.stderr)

    def test_valid_results_are_numerically_unchanged(self):
        # Keep the recorded valid report unchanged while rejecting bad inputs.
        self.assertEqual(DATA.with_suffix('.md').read_text(), self.run_report(self.rows).stdout)


if __name__ == '__main__':
    unittest.main()
