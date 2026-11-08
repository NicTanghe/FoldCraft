#!/usr/bin/env python3
"""Local, point-based EEVDF-inspired implementation scheduler (standard library only)."""
import argparse
from contextlib import contextmanager
import json
import math
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WEIGHTS = {'low': 1, 'normal': 2, 'high': 4, 'urgent': 8}
STATES = {'ready', 'active', 'review', 'done'}


def runnable(q):
    done = {t['id'] for t in q['tickets'] if t['status'] == 'done'}
    return [t for t in q['tickets'] if t['status'] in {'ready', 'active'}
            and set(t['depends_on']) <= done]


def virtual_time(q):
    tasks = runnable(q)
    return (sum(t['vruntime'] * WEIGHTS[t['priority']] for t in tasks)
            / sum(WEIGHTS[t['priority']] for t in tasks)) if tasks else q['clock']


def ranked(q):
    v = virtual_time(q)
    eligible = [t for t in runnable(q) if t['vruntime'] <= v + 1e-12]
    return sorted(eligible, key=lambda t: (
        t['vruntime'] + 1 / WEIGHTS[t['priority']],
        t['id']))


def validate(q):
    assert q['version'] == 1 and math.isfinite(q['clock']) and q['clock'] >= 0
    ids = [t['id'] for t in q['tickets']]
    assert len(ids) == len(set(ids)), 'Duplicate ticket ID'
    by_id = {t['id']: t for t in q['tickets']}
    for t in q['tickets']:
        assert t['priority'] in WEIGHTS and t['status'] in STATES
        assert t['title'].strip() and t['acceptance'].strip()
        for field in ['points', 'spent', 'vruntime']:
            assert math.isfinite(t[field]) and t[field] >= 0
        assert t['points'] > 0
        assert set(t['depends_on']) <= set(ids), 'Unknown dependency'
        assert bool(t['owner']) == (t['status'] == 'active'), 'Invalid owner'
        assert t['status'] != 'done' or t['pr'], 'Completed ticket needs merged PR'
    def visit(tid, path):
        assert tid not in path, 'Dependency cycle'
        for dep in by_id[tid]['depends_on']:
            visit(dep, path | {tid})
    for tid in ids:
        visit(tid, set())


@contextmanager
def locked(root):
    folder = root / 'comms'
    folder.mkdir(exist_ok=True)
    lock = folder / 'queue.lock'
    try:
        fd = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    except FileExistsError:
        raise ValueError('Queue writer is busy; inspect comms/queue.lock before retrying') from None
    try:
        with os.fdopen(fd, 'w') as f:
            f.write(str(os.getpid()) + '\n')
        yield
    finally:
        lock.unlink()


def apply(q, args):
    command = args.command
    if command == 'add':
        tid = f"FC-{max([int(t['id'][3:]) for t in q['tickets']] or [0]) + 1:04d}"
        q['tickets'].append(dict(id=tid, title=args.title, acceptance=args.acceptance,
            points=args.points, spent=0, priority=args.priority, depends_on=args.depends_on,
            status='ready', owner=None, vruntime=virtual_time(q), pr=None))
        return tid
    if command == 'start':
        # Active work keeps its entitlement; never silently over-dispatch around it.
        candidates = [t for t in ranked(q) if t['status'] == 'ready']
        if not candidates:
            raise ValueError('No eligible unowned ticket; active work may need a checkpoint')
        t = candidates[0]
        t['status'], t['owner'] = 'active', args.owner
        return t['id']
    t = next((t for t in q['tickets'] if t['id'] == args.ticket), None)
    if t is None:
        raise ValueError('Unknown ticket')
    if command == 'priority':
        # Preserve weighted lag when changing priority.
        v = virtual_time(q)
        t['vruntime'] = max(0, v - (v - t['vruntime']) * WEIGHTS[t['priority']] / WEIGHTS[args.priority])
        t['priority'] = args.priority
    elif command in {'charge', 'release', 'review'}:
        if t['status'] != 'active' or t['owner'] != args.owner:
            raise ValueError('Only the active ticket owner can checkpoint this ticket')
        if command == 'charge':
            if not math.isfinite(args.points) or args.points <= 0:
                raise ValueError('Charge must be finite and positive')
            t['spent'] += args.points
            t['vruntime'] += args.points / WEIGHTS[t['priority']]
        else:
            t['status'], t['owner'] = ('ready' if command == 'release' else 'review'), None
            if command == 'review':
                t['pr'] = args.pr
    elif command == 'reopen':
        if t['status'] != 'review':
            raise ValueError('Only review tickets can be reopened')
        t['vruntime'] = max(t['vruntime'], virtual_time(q))
        t['status'] = 'ready'
    elif command == 'done':
        if t['status'] != 'review' or not t['pr']:
            raise ValueError('Ticket must be in review with a PR')
        result = subprocess.run(['gh', 'pr', 'view', t['pr'], '--json', 'state,mergedAt'],
                                check=True, capture_output=True, text=True)
        pr = json.loads(result.stdout)
        if pr['state'] != 'MERGED' or not pr['mergedAt']:
            raise ValueError('PR has not been merged')
        t['status'] = 'done'
    q['clock'] = max(q['clock'], virtual_time(q))
    return t['id']


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', type=Path, default=ROOT, help='Dispatcher checkout containing the queue')
    commands = p.add_subparsers(dest='command', required=True)
    for name in ['list', 'next', 'check']:
        commands.add_parser(name)
    add = commands.add_parser('add')
    add.add_argument('title')
    add.add_argument('--acceptance', required=True)
    add.add_argument('--points', type=float, required=True)
    add.add_argument('--priority', choices=WEIGHTS, default='normal')
    add.add_argument('--depends-on', nargs='*', default=[])
    start = commands.add_parser('start')
    start.add_argument('--owner', required=True)
    for name in ['charge', 'release', 'review', 'reopen', 'done', 'priority']:
        cmd = commands.add_parser(name)
        cmd.add_argument('ticket')
        if name in {'charge', 'release', 'review'}:
            cmd.add_argument('--owner', required=True)
        if name == 'charge':
            cmd.add_argument('--points', type=float, required=True)
        if name == 'review':
            cmd.add_argument('--pr', required=True)
        if name == 'priority':
            cmd.add_argument('priority', choices=WEIGHTS)
    args = p.parse_args()
    try:
        with locked(args.root):
            path = args.root / 'implementation/queue.json'
            q = json.loads(path.read_text())
            validate(q)
            if args.command in {'list', 'next', 'check'}:
                result = q['tickets'] if args.command == 'list' else ranked(q) if args.command == 'next' else 'Queue valid'
            else:
                result = apply(q, args)
                validate(q)
                tmp = path.with_suffix('.tmp')
                tmp.write_text(json.dumps(q, indent=2, allow_nan=False) + '\n')
                tmp.replace(path)
            print(json.dumps(result, indent=2))
    except (ValueError, AssertionError, OSError, subprocess.CalledProcessError) as e:
        p.exit(1, f'factory: {str(e) or "Invalid queue data or argument"}\n')


if __name__ == '__main__':
    main()
