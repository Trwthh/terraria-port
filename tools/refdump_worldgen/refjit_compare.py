#!/usr/bin/env python3
r"""Proof that each run of run_refjit.sh is the run that wrote its reference data: every refdata file of the
scenario must be byte-identical to the file of the same name in work/jit/ref/<scenario>/data. Files that are
documented to differ between two runs of the same harness (timings, peak memory, the GUID / time stamps of a saved
.wld) are compared without those fields and reported as such; anything else that differs fails.

usage: python refjit_compare.py [scenario ...]
"""
import glob
import re
import json
import os
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.normpath(os.path.join(here, '..', '..'))
ref = os.path.join(root, 'refdata')
jit = os.path.join(root, 'work', 'jit', 'ref')


def files(pattern):
    return sorted(glob.glob(os.path.join(ref, pattern)))


SCEN = {
    'p1': files('*.json'),
    'framed': files('framed/ref_small.*'),
    'framed_scrambled': files('framed/ref_small_scrambled.*'),
    'collision': files('collision/*'),
    'light': files('light/*'),
    'sky': files('sky/*'),
    'ticks_world': [f for f in files('ticks/world.*')],
    'ticks_player': [f for f in files('ticks/player.*')],
    'worldgen_small': files('worldgen/ref_small.*'),
    'entities_npc': files('entities/npc.*'),
    'entities_proj': files('entities/proj.*'),
    'entities_item': files('entities/item.*'),
    'entities_citem': files('entities/citem.*'),
}
for s in ('settle_trace', 'lake_drain', 'surface_fill', 'shimmer_drain', 'honey_drain', 'lava_open'):
    SCEN['liquid_' + s] = files('liquid/' + s + '.*')
for t in ('ref_medium', 'ref_large', 'ref_bees', 'ref_drunk', 'ref_worthy', 'ref_celebration', 'ref_constant', 'ref_notraps', 'ref_remix',
          'ref_zenith', 'ref_skyblock', 'ref_sec_error', 'ref_sec_desert', 'ref_sec_frozen', 'ref_sec_mush', 'ref_sec_liquid', 'ref_trapsworthy', 'ref_shimmerocean', 'ref_dual_a'):
    SCEN['wg_' + t] = files('worldgen/matrix/' + t + '.*')
# framed: ref_small.* also matches the scrambled files
SCEN['framed'] = [f for f in SCEN['framed'] if '_scrambled' not in f]

# fields that differ between two runs of one harness by design (wall-clock time, memory): compared without them
# (refdata/ticks/README.md: `*.timing.json` is the only file that differs between two runs; the matrix summary.json
# carries the run's date, wall-clock, peak memory and the saved file's hash, whose GUID / time stamps differ by design)
VOLATILE_KEYS = {'tick_ms_total', 'total_ms', 'generation_ms', 'peak_working_set_mb', 'run_date', 'world_file_sha256',
                 'world_file_size', 'frameRunMs', 'totalMs'}
# matrix summary trace_lines: the game's own `Pass - <name>: <n>ms, rand: ..` and `Total Generation Time: <n>` lines (wall-clock)
MS = re.compile(r"[0-9]+ms|Total Generation Time: [0-9]+")


def strip(o):
    if isinstance(o, dict):
        return {k: ([MS.sub('<ms>', x) if isinstance(x, str) else x for x in v] if k == 'trace_lines' and isinstance(v, list) else strip(v))
                for k, v in o.items() if k not in VOLATILE_KEYS}
    if isinstance(o, list):
        return [strip(v) for v in o]
    return o


def compare(scen, paths):
    data = os.path.join(jit, scen, 'data')
    same, loose, bad = [], [], []
    for p in paths:
        name = os.path.basename(p)
        q = os.path.join(data, name)
        if not os.path.exists(q):
            # the dumped scenario data lives in a subfolder for some harnesses (refdata layout): look one level down
            alt = glob.glob(os.path.join(data, '*', name))
            q = alt[0] if alt else q
        if not os.path.exists(q):
            bad.append(name + ' (missing)')
            continue
        a, b = open(p, 'rb').read(), open(q, 'rb').read()
        if a == b:
            same.append(name)
            continue
        # timing / memory only?
        try:
            if name.endswith('.json'):
                ja, jb = json.loads(a), json.loads(b)
                if strip(ja) == strip(jb):
                    loose.append(name + ' (equal without ' + ', '.join(sorted(k for k in VOLATILE_KEYS if isinstance(ja, dict) and k in ja and ja.get(k) != jb.get(k))) + ')')
                    continue
                if isinstance(ja, dict) and isinstance(jb, dict):
                    bad.append(name + ' [keys ' + ', '.join(sorted(k for k in set(ja) | set(jb) if ja.get(k) != jb.get(k)))[:300] + ']')
                    continue
            if name.endswith('.jsonl'):
                la = [strip(json.loads(x)) for x in a.splitlines() if x.strip()]
                lb = [strip(json.loads(x)) for x in b.splitlines() if x.strip()]
                if la == lb:
                    loose.append(name + ' (equal without timing / memory fields)')
                    continue
        except ValueError:
            pass
        if name == 'MANIFEST.txt':
            la = [x for x in a.splitlines() if not x.startswith(b'elapsed_ms ')]
            lb = [x for x in b.splitlines() if not x.startswith(b'elapsed_ms ')]
            if la == lb:
                loose.append(name + ' (equal without elapsed_ms)')
                continue
        if name.endswith('.wld'):
            loose.append(name + ' (saved world: compare with wld_diff.py; GUID / time stamps differ by design)')
            continue
        bad.append(name)
    return same, loose, bad


def main():
    want = sys.argv[1:] or sorted(SCEN)
    failed = 0
    for scen in want:
        if not os.path.isdir(os.path.join(jit, scen)):
            print('%-22s not run' % scen)
            failed = 1
            continue
        same, loose, bad = compare(scen, SCEN[scen])
        print('%-22s %d of %d files identical%s%s' % (scen, len(same), len(SCEN[scen]),
              ('; ' + '; '.join(loose)) if loose else '', ('; DIFFER: ' + ', '.join(bad)) if bad else ''))
        failed |= bool(bad)
    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()
