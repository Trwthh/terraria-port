// node --no-warnings wasi-run.mjs <project root> <test.wasm> [test args...]
// Preopens: /w = the project root; /install = the player's Terraria install (TERRARIA_DIR, else the project's default),
// which the port only reads (its write guard refuses writes there: test sys::io::tests::the_install_folder_is_read_only).
import { readFileSync } from 'node:fs';
import { WASI } from 'node:wasi';
const [root, file, ...args] = process.argv.slice(2);
const wasi = new WASI({ version: 'preview1', args: [file, ...args], env: {}, preopens: { '/w': root, '/install': process.env.TERRARIA_DIR || 'C:/Users/user/Documents/Games/Terraria' }, returnOnExit: true });
const module = await WebAssembly.compile(readFileSync(file));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
const code = wasi.start(instance);
console.log(`[wasi-run] exit code ${code}`);
process.exitCode = code;
