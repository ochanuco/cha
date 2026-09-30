// Runs tests/vectors/*.json against the built wasm package (run scripts/build-wasm.sh first).
import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as cha from "../../pkg/cha_wasm.js";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const bytes = readFileSync(join(root, "pkg", "cha_wasm_bg.wasm"));
cha.initSync({ module: new WebAssembly.Module(bytes) });

const FUNCTIONS = [
  "blob_id",
  "prepare_revision",
  "prepare_restore",
  "prepare_conflict_resolution",
  "semantic_diff",
  "prepare_operation",
];

let failures = 0;
let passed = 0;

function fail(name, message) {
  failures += 1;
  console.error(`FAIL ${name}: ${message}`);
}

function invoke(fn, input) {
  if (fn === "blob_id") {
    return cha.blob_id(Uint8Array.from(Buffer.from(input.bytes_hex, "hex")));
  }
  return cha[fn](JSON.stringify(input));
}

function runCase(fn, testCase) {
  const name = `${fn}: ${testCase.name}`;
  let result;
  let thrown;
  try {
    result = invoke(fn, testCase.input);
  } catch (e) {
    thrown = e;
  }

  try {
    if ("ok" in testCase.expect) {
      if (thrown) {
        throw new Error(`threw ${thrown && thrown.code}: ${thrown && thrown.message}`);
      }
      assert.deepStrictEqual(fn === "blob_id" ? result : JSON.parse(result), testCase.expect.ok);
      assert.equal(invoke(fn, testCase.input), result, "output is not repeatable");
    } else {
      if (!thrown) {
        throw new Error("did not throw");
      }
      assert.ok(thrown instanceof Error, "thrown value is not an Error");
      assert.equal(thrown.name, "ChaError");
      assert.equal(typeof thrown.message, "string");
      assert.equal(thrown.code, testCase.expect.error.code);
      assert.deepStrictEqual(thrown.context, testCase.expect.error.context);
    }
    passed += 1;
  } catch (e) {
    fail(name, e.message);
  }
}

try {
  assert.equal(cha.abi_version(), "cha-abi/1");
  for (const fn of [...FUNCTIONS, "abi_version"]) {
    assert.equal(typeof cha[fn], "function", `missing export ${fn}`);
  }
  assert.equal(cha.blob_id(new Uint8Array(0)).length, "sha256:".length + 64);
  passed += 1;
} catch (e) {
  fail("exports", e.message);
}

const files = readdirSync(join(root, "tests", "vectors")).filter((f) => f.endsWith(".json")).sort();
assert.deepEqual(
  files.map((f) => f.replace(/\.json$/, "")).sort(),
  [...FUNCTIONS].sort(),
  "unexpected files in tests/vectors",
);

for (const file of files) {
  const doc = JSON.parse(readFileSync(join(root, "tests", "vectors", file), "utf8"));
  if (doc.abi !== "cha-abi/1") {
    fail(file, `unexpected abi ${doc.abi}`);
    continue;
  }
  for (const testCase of doc.cases) {
    runCase(doc.function, testCase);
  }
}

console.log(`${passed} passed, ${failures} failed`);
process.exit(failures === 0 ? 0 : 1);
