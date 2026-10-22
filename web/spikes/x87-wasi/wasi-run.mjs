// node --no-warnings wasi-run.mjs <project root> <test.wasm> [test args...]
import { readFileSync } from 'node:fs';
import { WASI } from 'node:wasi';
const [root, file, ...args] = process.argv.slice(2);
const wasi = new WASI({ version: 'preview1', args: [file, ...args], env: {}, preopens: { '/w': root }, returnOnExit: true });
const module = await WebAssembly.compile(readFileSync(file));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
const code = wasi.start(instance);
console.log(`[wasi-run] exit code ${code}`);
process.exitCode = code;
