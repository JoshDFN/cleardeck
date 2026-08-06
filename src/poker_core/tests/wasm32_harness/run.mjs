#!/usr/bin/env node
// Drives the wasm32 golden-vector harness with Node's plain WebAssembly API.
//
// Why Node: `wasm32-unknown-unknown` is the target the table canister is compiled
// to, and it has no WASI, so `wasmtime` cannot run it. Node's `WebAssembly` API
// instantiates it directly and this repo already requires Node for the frontend,
// so the wasm32 test job needs no new toolchain.
//
// Usage:
//   node run.mjs <module.wasm> check                 -> replay every vector
//   node run.mjs <module.wasm> regenerate            -> emit replacement S lines on stdout
//   node run.mjs <module.wasm> deck-order            -> emit create_deck() symbols
//   node run.mjs <module.wasm> shuffle <seed-hex>    -> emit the 52 symbols for one seed
//
// Exit status is 0 only when the requested operation succeeded. `check` exits 1
// with the failure report on stdout if any vector diverged.

import { readFile } from 'node:fs/promises';

const [wasmPath, mode, arg] = process.argv.slice(2);
if (!wasmPath || !mode) {
  console.error('usage: run.mjs <module.wasm> <check|regenerate|deck-order|shuffle> [seed-hex]');
  process.exit(2);
}

const bytes = await readFile(wasmPath);
const module = await WebAssembly.compile(bytes);

// A cdylib built from this crate imports nothing. Assert that rather than quietly
// stubbing imports: an unexpected import means the module is doing something the
// canister could not do, and the harness would stop speaking for the canister.
const imports = WebAssembly.Module.imports(module);
if (imports.length !== 0) {
  console.error(
    `refusing to run: the harness module imports ${imports.length} symbol(s): ` +
      imports.map((i) => `${i.module}.${i.name}`).join(', '),
  );
  process.exit(2);
}

const { exports } = await WebAssembly.instantiate(module, {});

const width = exports.pointer_width_bits();
if (width !== 32) {
  console.error(`refusing to run: pointer width inside the module is ${width}, expected 32`);
  process.exit(2);
}

const readReport = () => {
  const ptr = exports.report_ptr();
  const len = exports.report_len();
  const view = new Uint8Array(exports.memory.buffer, ptr, len);
  return new TextDecoder().decode(view);
};

// A Rust panic inside the module aborts the instance and surfaces here as a
// RuntimeError with no message. Say so, rather than letting the driver report an
// unexplained crash: a trap means poker_core rejected one of its own vectors.
const call = (fn, ...args) => {
  try {
    return fn(...args);
  } catch (err) {
    console.error(
      `the wasm module trapped: ${err}\n` +
        'A trap is a Rust panic inside poker_core (a bad vector line, or an input the\n' +
        'engine now refuses). Run the native test for a readable message:\n' +
        '  cargo test -p poker_core --test golden_vectors',
    );
    process.exit(1);
  }
};

switch (mode) {
  case 'check': {
    const failures = call(exports.run_golden);
    process.stdout.write(readReport());
    if (failures !== 0) {
      console.error(`${failures} vector(s) diverged on wasm32`);
      process.exit(1);
    }
    break;
  }
  case 'regenerate': {
    const lines = call(exports.regenerate_shuffle_vectors);
    const report = readReport();
    if (lines === 0 || report.length === 0) {
      console.error('regeneration produced nothing');
      process.exit(1);
    }
    process.stdout.write(report);
    break;
  }
  case 'deck-order': {
    call(exports.deck_order);
    process.stdout.write(readReport() + '\n');
    break;
  }
  case 'shuffle': {
    if (!arg || !/^([0-9a-fA-F]{2})+$/.test(arg)) {
      console.error('shuffle needs an even-length hex seed');
      process.exit(2);
    }
    const seed = Uint8Array.from(arg.match(/../g).map((h) => parseInt(h, 16)));
    if (seed.length > exports.input_cap()) {
      console.error('seed too long');
      process.exit(2);
    }
    new Uint8Array(exports.memory.buffer, exports.input_ptr(), seed.length).set(seed);
    call(exports.shuffle_seed, seed.length);
    process.stdout.write(readReport() + '\n');
    break;
  }
  default:
    console.error(`unknown mode ${mode}`);
    process.exit(2);
}
