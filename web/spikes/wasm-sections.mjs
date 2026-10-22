// node wasm-sections.mjs <file.wasm>: size of each section (code = id 10)
import { readFileSync } from 'node:fs';
const b = readFileSync(process.argv[2]); let p = 8; const names = {0:'custom',1:'type',2:'import',3:'function',5:'memory',6:'global',7:'export',9:'elem',10:'code',11:'data'};
const leb = () => { let r = 0, s = 0, x; do { x = b[p++]; r += (x & 0x7f) * 2 ** s; s += 7; } while (x & 0x80); return r; };
const out = {}; while (p < b.length) { const id = b[p++]; const n = leb(); const k = names[id] ?? id; out[k] = (out[k] ?? 0) + n; p += n; }
console.log(JSON.stringify({ file: process.argv[2].split(/[\/]/).pop(), total: b.length, ...out }));
