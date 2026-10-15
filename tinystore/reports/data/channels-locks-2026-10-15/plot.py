#!/usr/bin/env python3
"""Standalone research figure; bars are medians and whiskers observed ranges."""
import json
from pathlib import Path
from statistics import median

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent


def results(name):
    return [json.loads(x)['result'] for x in (ROOT / f'{name}.jsonl').read_text().splitlines()]


def bars(ax, labels, values, title, ylabel, log=False):
    mids = [median(v) for v in values]
    low = [m - min(v) for m, v in zip(mids, values)]
    high = [max(v) - m for m, v in zip(mids, values)]
    ax.bar(labels, mids, yerr=[low, high], capsize=4, color=['#427aa1', '#e58f65', '#6b9b73'][:len(labels)])
    ax.set_title(title, fontsize=11)
    ax.set_ylabel(ylabel)
    if log:
        ax.set_yscale('log')
    ax.grid(axis='y', alpha=.25)
    ax.set_axisbelow(True)


if __name__ == '__main__':
    channel = results('channels')
    locks = results('leases')
    resources = results('resources')
    indexes = results('indexes')
    kinds = ['broadcast', 'ring', 'queues']
    fig, axes = plt.subplots(2, 2, figsize=(11, 7))
    bars(axes[0, 0], kinds,
         [[r['delivery_p99_us'] / 1000 for r in channel if r['mode'] == 'paced' and r['subscribers'] == 1000 and r['kind'] == k] for k in kinds],
         '1000 subscribers, 1000 publishes/s (all loss = 0)', 'Delivery p99, ms (log)', True)
    bars(axes[0, 1], kinds,
         [[r['queued_memory']['rss_kib'] / 1024 for r in resources if r['experiment'] == 'channels' and r['subscribers'] == 1000 and r['payload_bytes'] == 64 and r['kind'] == k] for k in kinds],
         '1000 paused subscribers, 1024 retained slots', 'Live backlog RSS, MiB')
    lock_kinds = ['sql-poll', 'hybrid', 'admitted']
    bars(axes[1, 0], lock_kinds,
         [[r['attempts_per_success'] for r in locks if r['topology'] == 'hot' and r['clients'] == 1000 and r['kind'] == k] for k in lock_kinds],
         '1000 clients, one hot lease (4-second passes)', 'SQL attempts per grant (log)', True)
    bars(axes[1, 1], ['plain', 'FTS5', 'R*Tree'],
         [[r['insert_wal_bytes'] / 2**20 for r in indexes if r['kind'] == k] for k in ['plain', 'fts', 'rtree']],
         '100k synthetic rows, 1000-row FULL transactions', 'Insert WAL growth, MiB')
    fig.suptitle('TinyStore research prototypes — 15 October 2026', fontsize=15)
    fig.text(.5, .015, 'Linux VM · 2 CPU quota · bars: median · whiskers: observed min/max · no socket/SDK measurements', ha='center', fontsize=9)
    fig.tight_layout(rect=(0, .04, 1, .94))
    fig.savefig(ROOT / 'comparison.svg')
    svg = ROOT / 'comparison.svg'
    svg.write_text('\n'.join(line.rstrip() for line in svg.read_text().splitlines()) + '\n')
    fig.savefig(ROOT / 'comparison.png', dpi=180)
