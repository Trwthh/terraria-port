// The long stretches of a V8 CPU profile (work/logs/<name>.cpuprofile from run-game-headless.mjs "profile stop"):
// runs of samples without (idle) / (program) longer than --min ms, each with its inclusive time by function (the
// deepest frames that take most of it), so a frame that does not yield can be attributed.
//   node test/profile-slices.mjs work/logs/title_load.cpuprofile [--min 100] [--top 25]
import fs from 'node:fs';
const argv = process.argv.slice(2);
const opt = (n, d) => { const i = argv.indexOf('--' + n); return i >= 0 ? Number(argv[i + 1]) : d; };
const MIN = opt('min', 100);
const TOP = opt('top', 25);
const p = JSON.parse(fs.readFileSync(argv[0], 'utf8'));
const byId = new Map(p.nodes.map((n) => [n.id, n]));
const parent = new Map();
for (const n of p.nodes) for (const c of n.children ?? []) parent.set(c, n.id);
const name = (id) => byId.get(id).callFrame.functionName || '(anonymous)';
const idle = (id) => ['(idle)', '(program)', '(garbage collector)'].includes(name(id)) && !parent.has(id) === false && name(parent.get(id)) === '(root)';
// demangle-lite: the last path segments of a Rust v0 symbol
const short = (s) => {
  if (!s.startsWith('_R')) return s;
  const parts = [...s.matchAll(/\d+([a-z_][a-z0-9_]*)/gi)].map((m) => m[1]).filter((x) => !/^Cs|^Nt|^Ns/.test(x));
  return parts.slice(-3).join('::') || s;
};
let t = p.startTime;
const runs = [];
let cur = null;
p.samples.forEach((id, i) => {
  t += p.timeDeltas[i] ?? 0;
  const dt = (p.timeDeltas[i + 1] ?? 0) / 1000;
  if (idle(id)) {
    if (cur) runs.push(cur);
    cur = null;
    return;
  }
  cur ??= { start: t, ms: 0, samples: [] };
  cur.ms += dt;
  cur.samples.push([id, dt]);
});
if (cur) runs.push(cur);
const long = runs.filter((r) => r.ms >= MIN);
console.log(`${runs.length} busy runs, ${long.length} of ${MIN} ms or more`);
for (const r of long) {
  const incl = new Map();
  for (const [id, dt] of r.samples) {
    const seen = new Set();
    for (let k = id; k !== undefined; k = parent.get(k)) {
      const f = short(name(k));
      if (!seen.has(f)) { seen.add(f); incl.set(f, (incl.get(f) ?? 0) + dt); }
    }
  }
  console.log(`\n== run at ${((r.start - p.startTime) / 1000).toFixed(0)} ms: ${r.ms.toFixed(0)} ms`);
  for (const [f, ms] of [...incl].sort((a, b) => b[1] - a[1]).slice(0, TOP)) console.log(`${ms.toFixed(0).padStart(7)} ${f}`);
}
