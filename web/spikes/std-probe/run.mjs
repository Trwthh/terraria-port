// node run.mjs <file.wasm> : calls every export, prints value or the trap
import { readFileSync } from 'node:fs';
const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), {});
for (const [name, f] of Object.entries(instance.exports)) {
  if (typeof f !== 'function') continue;
  try { const v = f(); console.log(`${name.padEnd(22)} returned ${typeof v === 'number' ? '0x' + (v >>> 0).toString(16) : v}`); }
  catch (e) { console.log(`${name.padEnd(22)} TRAP: ${e.message}`); }
}
