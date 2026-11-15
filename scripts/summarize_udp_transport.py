#!/usr/bin/env python3
"""Summarize benchmark CSV; reject malformed or incomplete timing comparisons."""
import csv
import statistics
import sys
from collections import defaultdict

if len(sys.argv) != 2:
    raise SystemExit('usage: summarize_udp_transport.py result.csv (one workload per report)')
rows = []
for path in sys.argv[1:]:
    with open(path) as source:
        reader = csv.DictReader(line for line in source if not line.startswith('#'))
        for row in reader:
            if None in row or any(value is None for value in row.values()):
                raise SystemExit(f'malformed CSV in {path}: {row}')
            rows.append(row)
if not rows:
    raise SystemExit('usage: summarize_udp_transport.py result.csv')
scenarios = {row.get('scenario', 'synthetic-pairs') for row in rows}
if len(scenarios) != 1:
    raise SystemExit('summarize one avatar scenario at a time')
scenario = next(iter(scenarios))
print(f'Scenario: {scenario}. CPU times cover the whole run, including the drain interval.\n')
count_label = 'Participants' if scenario != 'synthetic-pairs' else 'Pairs'
groups = defaultdict(list)
for row in rows:
    groups[(int(row['pairs']), row['mode'], row['kind'])].append(row)
# Validate identity before reporting/filtering any comparisons. A single-mode
# report is valid; if both comparison modes exist their measurement kinds and
# trial sets must agree, including lossy trials.
for key, dataset in groups.items():
    ids = [row['trial'] for row in dataset]
    if len(ids) != len(set(ids)):
        raise SystemExit(f'duplicate trial IDs in {key}')
for pairs in {p for p, _, _ in groups}:
    separate = {kind for p, mode, kind in groups if p == pairs and mode == 'separate'}
    shared = {kind for p, mode, kind in groups if p == pairs and mode == 'shared'}
    if separate and shared:
        if separate != shared:
            raise SystemExit(f'unpaired measurement kinds for {pairs}')
        for kind in separate:
            a = {row['trial'] for row in groups[pairs, 'separate', kind]}
            b = {row['trial'] for row in groups[pairs, 'shared', kind]}
            if a != b:
                raise SystemExit(f'unpaired trial IDs for {pairs}, {kind}')
print(f'| {count_label} | Mode | Replay ns/packet, median [min–max] | Receiver CPU ms / whole run, median [min–max] | UDP p95 µs, median | Drops / send errors | Scratch bytes |')
print('| --- | --- | --- | --- | --- | --- | --- |')
for pairs, mode in sorted({(p, m) for p, m, _ in groups}):
    replay = groups[pairs, mode, 'replay']
    udp = groups.get((pairs, mode, 'udp'), [])
    ns = [float(row['ns_per_packet']) for row in replay]
    # Lossy runs are visible, but cannot qualify as a throughput/CPU speedup.
    clean = [row for row in udp if all(int(row[key]) == 0 for key in ['dropped', 'send_errors', 'invalid', 'duplicate'])]
    cpu = [float(row['cpu_ms']) for row in clean]
    latency = [float(row['latency_p95_us']) for row in udp]
    def spread(values):
        return f'{statistics.median(values):.3f} [{min(values):.3f}–{max(values):.3f}]' if values else '—'
    print(f"| {pairs} | {mode} | {spread(ns)} | {spread(cpu)} | {statistics.median(latency) if latency else float('nan'):.1f} | {sum(int(r['dropped']) for r in udp)} / {sum(int(r['send_errors']) for r in udp)} | {(replay or udp)[0]['scratch_bytes']} |")
print('\nShared minus separate time; negative means shared took less time:')
for pairs in sorted({p for p, _, _ in groups}):
    for kind, field in [('replay', 'ns_per_packet'), ('udp', 'cpu_ms')]:
        a = groups.get((pairs, 'separate', kind), [])
        b = groups.get((pairs, 'shared', kind), [])
        if not a or not b:
            continue
        by_trial = {row['trial']: row for row in b}
        changes = []
        for row in a:
            other = by_trial.get(row['trial'])
            if other is None:
                raise SystemExit('unpaired trial')
            if kind == 'udp' and any(int(r[key]) != 0 for r in [row, other] for key in ['dropped', 'send_errors', 'invalid', 'duplicate']):
                continue
            changes.append((float(other[field]) / float(row[field]) - 1) * 100)
        if changes:
            print(f'  pairs={pairs} {kind}: paired median {statistics.median(changes):+.2f}%, range [{min(changes):+.2f}%, {max(changes):+.2f}%], n={len(changes)}')
