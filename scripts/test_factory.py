import argparse
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('factory', Path(__file__).with_name('factory.py'))
f = importlib.util.module_from_spec(spec)
spec.loader.exec_module(f)


def command(q, name, **kwargs):
    result = f.apply(q, argparse.Namespace(command=name, **kwargs))
    f.validate(q)
    return result


def add(q, priority='normal', depends_on=None):
    return command(q, 'add', title='A feature', acceptance='Observable acceptance',
                   points=3, priority=priority, depends_on=depends_on or [])


class FactoryTests(unittest.TestCase):
    def setUp(self):
        self.q = dict(version=1, clock=0, tickets=[])

    def test_boost_and_long_run_weighted_service(self):
        low = add(self.q, 'low')
        urgent = add(self.q, 'urgent')
        self.assertEqual(f.ranked(self.q)[0]['id'], urgent)
        counts = {low: 0, urgent: 0}
        for _ in range(900):
            tid = command(self.q, 'start', owner='worker')
            command(self.q, 'charge', ticket=tid, owner='worker', points=1)
            command(self.q, 'release', ticket=tid, owner='worker')
            counts[tid] += 1
        self.assertLessEqual(abs(counts[urgent] - 8 * counts[low]), 9)
        self.assertGreater(counts[low], 90)

    def test_dependencies_and_cycle_rejection(self):
        first = add(self.q)
        second = add(self.q, 'urgent', [first])
        self.assertEqual([t['id'] for t in f.ranked(self.q)], [first])
        self.q['tickets'][0].update(status='done', pr='https://example.test/pr/1')
        self.assertEqual(f.ranked(self.q)[0]['id'], second)
        self.q['tickets'][0]['depends_on'] = [second]
        with self.assertRaisesRegex(AssertionError, 'cycle'):
            f.validate(self.q)

    def test_owner_prevents_duplicate_start_and_unauthorized_charge(self):
        tid = add(self.q)
        command(self.q, 'start', owner='alice')
        with self.assertRaises(ValueError):
            command(self.q, 'start', owner='bob')
        before = copy.deepcopy(self.q)
        with self.assertRaises(ValueError):
            command(self.q, 'charge', ticket=tid, owner='bob', points=1)
        self.assertEqual(self.q, before)

    def test_new_arrival_and_priority_change(self):
        tid = add(self.q)
        command(self.q, 'start', owner='alice')
        command(self.q, 'charge', ticket=tid, owner='alice', points=4)
        newcomer = add(self.q)
        self.assertEqual(self.q['tickets'][1]['vruntime'], 2)
        command(self.q, 'priority', ticket=newcomer, priority='urgent')
        self.assertEqual(f.ranked(self.q)[0]['id'], newcomer)

    def test_done_requires_actual_merge(self):
        tid = add(self.q)
        command(self.q, 'start', owner='alice')
        command(self.q, 'review', ticket=tid, owner='alice', pr='https://example.test/pr/1')
        with patch.object(f.subprocess, 'run') as run:
            run.return_value.stdout = json.dumps(dict(state='OPEN', mergedAt=None))
            with self.assertRaisesRegex(ValueError, 'not been merged'):
                command(self.q, 'done', ticket=tid)
            self.assertEqual(self.q['tickets'][0]['status'], 'review')
            run.return_value.stdout = json.dumps(dict(state='MERGED', mergedAt='2026-11-08'))
            command(self.q, 'done', ticket=tid)
        self.assertEqual(self.q['tickets'][0]['status'], 'done')

    def test_review_reopen(self):
        tid = add(self.q)
        command(self.q, 'start', owner='alice')
        command(self.q, 'review', ticket=tid, owner='alice', pr='https://example.test/pr/1')
        command(self.q, 'reopen', ticket=tid)
        self.assertEqual(command(self.q, 'start', owner='bob'), tid)

    def test_lock_exclusion_and_cleanup(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with f.locked(root):
                with self.assertRaises(ValueError):
                    with f.locked(root):
                        self.fail('Second writer entered')
            self.assertFalse((root / 'comms/queue.lock').exists())

    def test_invalid_cli_add_preserves_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'implementation').mkdir()
            path = root / 'implementation/queue.json'
            original = json.dumps(self.q)
            path.write_text(original)
            result = subprocess.run(['python3', str(Path(f.__file__)), '--root', tmp,
                                     'add', 'Invalid', '--points', 'nan', '--acceptance', 'Result'],
                                    capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(path.read_text(), original)


if __name__ == '__main__':
    unittest.main()
