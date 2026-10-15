#!/usr/bin/env python3
"""Audit the evidence and derive every comparison table without old-round data."""
import json
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def read(name, expected):
    records = [json.loads(x) for x in (ROOT / (name + '.jsonl')).read_text().splitlines()]
    assert len(records) == expected, (name, len(records), expected)
    for record in records:
        assert record.get('exit', 0) == 0, record
        assert record['start_utc'] <= record['end_utc']
        assert record['result'].get('status', 'pass') == 'pass', record
    return records


def table(headers, rows):
    values = [[str(x) for x in headers]] + [[str(x) for x in row] for row in rows]
    widths = [max(3, max(len(r[i]) for r in values)) for i in range(len(headers))]
    line = lambda r: '| ' + ' | '.join(x.ljust(w) for x, w in zip(r, widths)) + ' |'
    return '\n'.join([line(values[0]), line(['-' * w for w in widths])] + [line(r) for r in values[1:]])


def passes(rows, getter, digits=1):
    return '; '.join(f'{getter(r):,.{digits}f}' for r in rows)


def group(records, fields):
    groups = defaultdict(list)
    for record in records:
        r = record['result']
        groups[tuple(r[x] for x in fields)].append(r)
    return groups


if __name__ == '__main__':
    main = read('channels', 81)
    resources = read('resources', 48)
    locks = read('leases', 72)
    controls = read('lease-controls', 28)
    indexes = read('indexes', 9)
    preflight = read('preflight', 13)
    killed = read('kill', 6)
    soaked = read('soak', 10)
    waiter_memory = read('waiter-memory', 12)
    timelines = [*main, *resources, *locks, *controls, *indexes, *preflight, *killed, *soaked, *waiter_memory]
    ordered = sorted(timelines, key=lambda r: r['start_utc'])
    for first, second in zip(ordered, ordered[1:]):
        assert first['end_utc'] <= second['start_utc'], ('overlap', first['label'], second['label'])
    for record in [*main, *resources]:
        r = record['result']
        if r['experiment'] != 'channels':
            continue
        assert r['delivered'] + r['lost'] == r['messages'] * r['subscribers']
        if r['kind'] == 'queues':
            assert r['lost'] == r['queue_rejected']
    tables = {}
    cg = group(main, ('mode', 'subscribers', 'kind'))
    for mode, name in [('paced', 'CHANNEL_PACED'), ('saturate', 'CHANNEL_SATURATION')]:
        rows = []
        for n in (1, 100, 1000):
            for kind in ('broadcast', 'ring', 'queues'):
                rs = cg[mode, n, kind]
                assert len(rs) == 3
                rows.append([n, kind,
                    passes(rs, lambda r: r['publish_per_s'] / 1e6, 3),
                    passes(rs, lambda r: r['deliveries_per_s'] / 1e6, 3),
                    passes(rs, lambda r: 100 * r['lost'] / (r['subscribers'] * r['messages']), 2),
                    passes(rs, lambda r: r['delivery_p50_us'] / 1000, 3),
                    passes(rs, lambda r: r['delivery_p99_us'] / 1000, 3)])
        tables[name] = table(['Subscribers', 'Algorithm', 'Publish M/s', 'Delivered M/s', 'Lost %', 'Delivery p50 ms', 'Delivery p99 ms'], rows)
    rows = []
    for n in (1, 100, 1000):
        for kind in ('broadcast', 'ring', 'queues'):
            rs = cg['slow', n, kind]
            slow = (n + 9) // 10
            fast = n - slow
            rows.append([n, kind, passes(rs, lambda r: r['publish_per_s'] / 1e6, 3),
                passes(rs, lambda r: r['deliveries_per_s'] / 1e6, 3),
                passes(rs, lambda r: 100 * r['fast_lost'] / (fast * r['messages']), 2) if fast else 'no fast reader',
                passes(rs, lambda r: 100 * r['slow_lost'] / (slow * r['messages']), 2),
                passes(rs, lambda r: r['delivery_p99_us'] / 1000, 2)])
    tables['CHANNEL_SLOW'] = table(['Subscribers', 'Algorithm', 'Publish M/s', 'Delivered M/s', 'Fast lost %', 'Slow lost %', 'Delivery p99 ms'], rows)
    mg = group([r for r in resources if r['result']['experiment'] == 'channels'], ('subscribers', 'payload_bytes', 'kind'))
    rows = []
    for n in (1, 100, 1000):
        for size in (64, 65536):
            for kind in ('broadcast', 'ring', 'queues'):
                rs = mg[n, size, kind]
                assert len(rs) == 2
                rows.append([n, size, kind, passes(rs, lambda r: r['idle_memory']['rss_kib'] / 1024, 2),
                    passes(rs, lambda r: r['queued_memory']['rss_kib'] / 1024, 2),
                    passes(rs, lambda r: r['final_memory']['peak_rss_kib'] / 1024, 2)])
    tables['CHANNEL_MEMORY'] = table(['Subscribers', 'Payload B', 'Algorithm', 'Idle RSS MiB', 'Backlog RSS MiB', 'Process HWM MiB'], rows)
    rg = group([r for r in resources if r['result']['experiment'] == 'route'], ('subscriptions', 'kind'))
    rows = []
    for n in (1100, 10100, 100100):
        for kind in ('scan', 'trie'):
            rs = rg[n, kind]
            rows.append([n, kind, passes(rs, lambda r: r['lookup_per_s'], 0)])
    tables['ROUTING'] = table(['Registrations', 'Algorithm', 'Lookups/s'], rows)
    lg = group(locks, ('topology', 'clients', 'kind'))
    for topology, name in [('hot', 'LEASE_HOT'), ('spread', 'LEASE_SPREAD')]:
        rows = []
        for n in (10, 100, 1000):
            for kind in ('kv-poll', 'sql-poll', 'hybrid', 'admitted'):
                rs = lg[topology, n, kind]
                assert len(rs) == 3
                rows.append([n, kind, passes(rs, lambda r: r['cycles_per_s'], 0),
                    passes(rs, lambda r: r['attempts_per_success'], 1),
                    passes(rs, lambda r: r['wait_p50_us'] / 1000, 2),
                    passes(rs, lambda r: r['wait_p99_us'] / 1000, 2),
                    passes(rs, lambda r: r['clients_with_zero_success'], 0),
                    passes(rs, lambda r: r['cpu_s'], 2)])
        tables[name] = table(['Clients', 'Algorithm', 'Cycles/s', 'Attempts/grant', 'Wait p50 ms', 'Wait p99 ms', 'Zero success', 'CPU seconds'], rows)
    controlg = group(controls, ('topology', 'permits', 'batch_limit', 'durability', 'kind', 'renew_per_cycle'))
    rows, renewal_rows = [], []
    for key, rs in sorted(controlg.items()):
        topology, permits, batch, durability, kind, renewal = key
        assert len(rs) == 2
        if not renewal:
            rows.append([topology, permits, batch, durability, kind,
                passes(rs, lambda r: r['cycles_per_s'], 0),
                passes(rs, lambda r: r['attempts_per_success'], 1),
                passes(rs, lambda r: r['mutations'] / max(1, r['cycles']), 1),
                passes(rs, lambda r: r['wal_bytes'] / 2**20, 2)])
        else:
            renewal_rows.append([kind, passes(rs, lambda r: r['cycles_per_s'], 0),
                passes(rs, lambda r: r['renew_p50_us'] / 1000, 3),
                passes(rs, lambda r: r['renew_p99_us'] / 1000, 3),
                passes(rs, lambda r: r['mutations'] / max(1, r['cycles']), 1),
                passes(rs, lambda r: r['wal_bytes'] / 2**20, 2)])
    tables['LEASE_CONTROLS'] = table(['Topology', 'Permits', 'Batch', 'Sync', 'Algorithm', 'Cycles/s', 'Attempts/grant', 'Affected rows/cycle', 'Live WAL MiB'], rows)
    tables['LEASE_RENEW'] = table(['Algorithm (100 spread clients)', 'Cycles/s with renewal', 'Renew p50 ms', 'Renew p99 ms', 'Affected rows/cycle', 'Live WAL MiB'], renewal_rows)
    ig = group(indexes, ('kind',))
    rows, mutations, storage, rebuild = [], [], [], []
    for kind in ('plain', 'fts', 'rtree'):
        rs = ig[kind,]
        assert len(rs) == 3
        rows.append([kind, passes(rs, lambda r: r['insert_per_s'], 0),
            passes(rs, lambda r: r['batch_p99_us'] / 1000, 2),
            passes(rs, lambda r: r['query_p50_us'] / 1000, 3),
            passes(rs, lambda r: r['query_p99_us'] / 1000, 3),
            passes(rs, lambda r: r['insert_wal_bytes'] / 2**20, 2),
            passes(rs, lambda r: r['final_memory']['peak_rss_kib'] / 1024, 2)])
        mutations.append([kind, passes(rs, lambda r: r['update_s'] * 1000, 1),
            passes(rs, lambda r: r['update_wal_bytes'] / 2**20, 2),
            passes(rs, lambda r: r['delete_s'] * 1000, 1),
            passes(rs, lambda r: r['delete_wal_bytes'] / 2**20, 2)])
        names = {v['name'] for r in rs for v in r['inserted_dbstat']}
        for name in sorted(names):
            storage.append([kind, name, passes(rs, lambda r: next((v['bytes'] for v in r['inserted_dbstat'] if v['name'] == name), 0), 0)])
        storage.append([kind, 'total allocated objects', passes(rs, lambda r: sum(v['bytes'] for v in r['inserted_dbstat']), 0)])
        if kind != 'plain':
            rebuild.append([kind, passes(rs, lambda r: r['rebuild']['duration_s'] * 1000, 1),
                passes(rs, lambda r: r['rebuild']['wal_bytes'] / 2**20, 2),
                passes(rs, lambda r: r['rebuild']['held_reader_queries'], 0),
                passes(rs, lambda r: r['rebuild']['held_reader_p99_us'] / 1000, 3),
                '; '.join(f"{r['rebuild']['checkpoint_done_frames']}/{r['rebuild']['checkpoint_log_frames']}" for r in rs),
                passes(rs, lambda r: r['rebuild']['dual_write_p99_us'] / 1000, 3)])
    tables['INDEX_COST'] = table(['Index', 'Insert rows/s', '1000-row batch p99 ms', 'Query p50 ms', 'Query p99 ms', 'Insert WAL MiB', 'Process HWM MiB'], rows)
    tables['INDEX_MUTATIONS'] = table(['Index', '10k update ms', 'Update WAL MiB', '10k delete ms', 'Delete WAL MiB'], mutations)
    tables['INDEX_STORAGE'] = table(['Index case', 'Object', 'Allocated bytes after insertion'], storage)
    tables['INDEX_REBUILD'] = table(['Index', 'Writer transaction ms', 'Rebuild WAL MiB', 'Held reader queries', 'Reader p99 ms', 'Checkpoint done/log frames', 'Dual-write insert p99 ms'], rebuild)
    soak_channels = [r for r in soaked if r['result']['experiment'] == 'channels']
    sg = group(soak_channels, ('kind',))
    rows = []
    for kind in ('broadcast', 'ring', 'queues'):
        rs = sg[kind,]
        rows.append([kind, passes(rs, lambda r: r['publish_s'], 2),
            passes(rs, lambda r: r['deliveries_per_s'] / 1e6, 3),
            passes(rs, lambda r: r['lost'], 0),
            passes(rs, lambda r: r['delivery_p99_us'] / 1000, 3),
            passes(rs, lambda r: r['final_memory']['peak_rss_kib'] / 1024, 2)])
    tables['CHANNEL_SOAK'] = table(['Algorithm', 'Publish seconds', 'Delivered M/s', 'Lost events', 'Delivery p99 ms', 'Process HWM MiB'], rows)
    sg = group([r for r in soaked if r['result']['experiment'] == 'leases'], ('kind',))
    rows = []
    for kind in ('sql-poll', 'admitted'):
        rs = sg[kind,]
        rows.append([kind, passes(rs, lambda r: r['cycles_per_s'], 1),
            passes(rs, lambda r: r['attempts_per_success'], 1),
            passes(rs, lambda r: r['clients_with_zero_success'], 0),
            passes(rs, lambda r: r['wait_p99_us'] / 1e6, 3),
            passes(rs, lambda r: r['cpu_s'], 2),
            passes(rs, lambda r: r['wal_bytes'] / 2**20, 2)])
    tables['LEASE_SOAK'] = table(['Algorithm', 'Cycles/s', 'Attempts/grant', 'Zero success', 'Wait p99 s', 'CPU seconds', 'Live WAL MiB'], rows)
    wg = group(waiter_memory, ('waiters','kind'))
    rows = []
    for n in (10,100,1000):
        for kind in ('fifo','notify'):
            rs=wg[n,kind]
            assert len(rs)==2
            rows.append([n,kind,passes(rs,lambda r:r['before_waiters']['rss_kib']/1024,2),
                passes(rs,lambda r:r['idle_waiters']['rss_kib']/1024,2),
                passes(rs,lambda r:r['after_cancel']['rss_kib']/1024,2),
                passes(rs,lambda r:r['after_cancel']['peak_rss_kib']/1024,2)])
    tables['LEASE_MEMORY'] = table(['Waiters','Admission','Before RSS MiB','Idle waiters RSS MiB','After cancellation RSS MiB','Process HWM MiB'],rows)
    timeline_rows = []
    for name, records in [('preflight', preflight), ('kill', killed), ('channels', main), ('resources', resources),
                          ('leases', locks), ('lease-controls', controls), ('indexes', indexes), ('soak', soaked), ('waiter-memory',waiter_memory)]:
        timeline_rows.append([name, len(records), records[0]['start_utc'], records[-1]['end_utc']])
    tables['TIMELINE'] = table(['Suite', 'Cases', 'Start UTC', 'End UTC'], timeline_rows)
    (ROOT / 'tables.json').write_text(json.dumps(tables, indent=2) + '\n')
    (ROOT / 'tables.md').write_text('\n\n'.join(f'## {key}\n\n{value}' for key, value in tables.items()) + '\n')
    print(tables['TIMELINE'])
    print('Audited', len(timelines), 'non-overlapping successful processes.')
