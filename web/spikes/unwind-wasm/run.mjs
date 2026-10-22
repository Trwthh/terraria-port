// node run.mjs <unwind_wasm.wasm>
import { readFileSync } from 'node:fs';
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), {});
for (const [name, args, want] of [['try_catch', [], 1], ['bounds_check', [1], 2], ['bounds_check', [5], 99]]) {
  try { const v = instance.exports[name](...args); console.log(`${name}(${args}) = ${v} ${v === want ? 'OK' : 'WRONG'}`); }
  catch (e) { console.log(`${name}(${args}) TRAP: ${e.message}`); }
}
