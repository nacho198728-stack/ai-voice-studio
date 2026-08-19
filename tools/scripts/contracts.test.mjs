import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const toolPath = fileURLToPath(new URL("./contracts.mjs", import.meta.url));

function writeFixture(root, { version = validVersion(), errorCodes = validErrorCodes() } = {}) {
  writeFileSync(path.join(root, "VERSION"), "0.0.0\n");
  const contracts = path.join(root, "core", "contracts");
  const rustSource = path.join(contracts, "src");
  const cppInclude = path.join(contracts, "include", "ai_voice_contracts");
  for (const directory of [contracts, rustSource, cppInclude]) {
    mkdirSync(directory, { recursive: true });
  }
  writeFileSync(path.join(contracts, "version.json"), `${JSON.stringify(version, null, 2)}\n`);
  writeFileSync(
    path.join(contracts, "error-codes.json"),
    `${JSON.stringify(errorCodes, null, 2)}\n`,
  );
}

function validVersion() {
  return {
    schema_version: 1,
    runtime: { version: "0.0.0" },
    ipc_protocol: { current_version: 1, minimum_compatible_version: 1 },
    voice_engine_abi: { v1_version: 1, current_version: 1, minimum_compatible_version: 1 },
    compatibility_rule:
      "A peer is compatible if and only if its integer version is in the inclusive range [minimum_compatible_version, current_version].",
  };
}

function validErrorCodes() {
  return {
    schema_version: 1,
    numeric_ranges: [
      { name: "success", start: 0, end: 0 },
      { name: "version", start: 1000, end: 1099 },
      { name: "framing", start: 1100, end: 1199 },
      { name: "runtime", start: 1200, end: 1299 },
      { name: "engine", start: 1300, end: 1399 },
      { name: "internal", start: 1900, end: 1999 },
    ],
    codes: [
      { name: "Success", value: 0, category: "success", meaning: "The operation completed successfully." },
      { name: "UnsupportedProtocolVersion", value: 1000, category: "version", meaning: "The peer IPC protocol version is incompatible." },
      { name: "UnsupportedVoiceEngineAbi", value: 1001, category: "version", meaning: "The VoiceEngine ABI version is incompatible." },
      { name: "MalformedFrame", value: 1100, category: "framing", meaning: "The received control frame is malformed." },
      { name: "FrameTooLarge", value: 1101, category: "framing", meaning: "The received control frame exceeds the configured limit." },
      { name: "RuntimeUnavailable", value: 1200, category: "runtime", meaning: "The isolated runtime is unavailable." },
      { name: "RuntimeShuttingDown", value: 1201, category: "runtime", meaning: "The isolated runtime is shutting down." },
      { name: "EngineUnavailable", value: 1300, category: "engine", meaning: "No compatible VoiceEngine is available." },
      { name: "InvalidArgument", value: 1301, category: "engine", meaning: "A VoiceEngine ABI argument, structure prefix, or buffer view is invalid." },
      { name: "InvalidState", value: 1302, category: "engine", meaning: "The VoiceEngine operation is not valid in the handle's current lifecycle state." },
      { name: "BufferTooSmall", value: 1303, category: "engine", meaning: "A caller-owned VoiceEngine output buffer lacks the required capacity." },
      { name: "InternalError", value: 1900, category: "internal", meaning: "An unexpected internal boundary failure occurred." },
    ],
  };
}

function run(root, mode) {
  return execFileSync(process.execPath, [toolPath, mode, "--root", root], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function runFailure(root, mode) {
  try {
    run(root, mode);
    assert.fail(`${mode} unexpectedly succeeded`);
  } catch (error) {
    assert.equal(error.status, 1);
    return error.stderr;
  }
}

function fixture(t, options) {
  const root = mkdtempSync(path.join(os.tmpdir(), "aivs-contracts-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  writeFixture(root, options);
  return root;
}

test("generate produces repeatable mappings accepted by read-only check", (t) => {
  const root = fixture(t);

  run(root, "generate");
  const rustPath = path.join(root, "core/contracts/src/generated.rs");
  const cppPath = path.join(root, "core/contracts/include/ai_voice_contracts/generated_contracts.hpp");
  const cPath = path.join(
    root,
    "core/contracts/include/ai_voice_contracts/generated_contracts_c.h",
  );
  const firstRust = readFileSync(rustPath, "utf8");
  const firstCpp = readFileSync(cppPath, "utf8");
  const firstC = readFileSync(cPath, "utf8");

  assert.doesNotThrow(() => run(root, "check"));
  run(root, "generate");
  assert.equal(readFileSync(rustPath, "utf8"), firstRust);
  assert.equal(readFileSync(cppPath, "utf8"), firstCpp);
  assert.equal(readFileSync(cPath, "utf8"), firstC);
  assert.match(firstC, /AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION/);
  assert.match(firstC, /AIVS_ERROR_INVALID_ARGUMENT/);
});

test("check detects stale generated output without rewriting it", (t) => {
  const root = fixture(t);
  run(root, "generate");
  const rustPath = path.join(root, "core/contracts/src/generated.rs");
  writeFileSync(rustPath, "stale mapping\n");

  assert.match(runFailure(root, "check"), /generated mapping drift/);
  assert.equal(readFileSync(rustPath, "utf8"), "stale mapping\n");
});

test("check rejects duplicate error names and values", (t) => {
  const duplicateName = validErrorCodes();
  duplicateName.codes[1].name = "Success";
  const duplicateNameRoot = fixture(t, { errorCodes: duplicateName });
  assert.match(runFailure(duplicateNameRoot, "check"), /duplicate error code name/);

  const duplicate = validErrorCodes();
  duplicate.codes[1].value = 0;
  const duplicateRoot = fixture(t, { errorCodes: duplicate });
  assert.match(runFailure(duplicateRoot, "check"), /duplicate error code value/);
});

test("check rejects invalid compatibility, Runtime-version drift, and malformed JSON", (t) => {
  const invalidSchema = validVersion();
  invalidSchema.schema_version = 2;
  const invalidSchemaRoot = fixture(t, { version: invalidSchema });
  assert.match(runFailure(invalidSchemaRoot, "check"), /schema_version must be 1/);

  const invalidVersion = validVersion();
  invalidVersion.ipc_protocol.minimum_compatible_version = 2;
  const invalidRangeRoot = fixture(t, { version: invalidVersion });
  assert.match(runFailure(invalidRangeRoot, "check"), /minimum compatible version/);

  const runtimeDrift = validVersion();
  runtimeDrift.runtime.version = "0.0.1";
  const runtimeDriftRoot = fixture(t, { version: runtimeDrift });
  assert.match(runFailure(runtimeDriftRoot, "check"), /differs from root VERSION/);

  const malformedRoot = fixture(t);
  writeFileSync(path.join(malformedRoot, "core/contracts/error-codes.json"), "{ not JSON\n");
  assert.match(runFailure(malformedRoot, "check"), /invalid JSON/);
});
