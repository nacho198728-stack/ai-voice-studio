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

function writeFixture(
  root,
  { version = validVersion(), errorCodes = validErrorCodes(), runtimeMessage = validRuntimeMessage() } = {},
) {
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
  writeFileSync(
    path.join(contracts, "runtime-message-v1.json"),
    `${JSON.stringify(runtimeMessage, null, 2)}\n`,
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

function validRuntimeMessage() {
  return JSON.parse(
    readFileSync(
      fileURLToPath(new URL("../../core/contracts/runtime-message-v1.json", import.meta.url)),
      "utf8",
    ),
  );
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
  const runtimeRustPath = path.join(root, "core/contracts/src/runtime_message_generated.rs");
  const runtimeCppPath = path.join(
    root,
    "core/contracts/include/ai_voice_contracts/runtime_message_generated.hpp",
  );
  const firstRust = readFileSync(rustPath, "utf8");
  const firstCpp = readFileSync(cppPath, "utf8");
  const firstC = readFileSync(cPath, "utf8");
  const firstRuntimeRust = readFileSync(runtimeRustPath, "utf8");
  const firstRuntimeCpp = readFileSync(runtimeCppPath, "utf8");

  assert.doesNotThrow(() => run(root, "check"));
  run(root, "generate");
  assert.equal(readFileSync(rustPath, "utf8"), firstRust);
  assert.equal(readFileSync(cppPath, "utf8"), firstCpp);
  assert.equal(readFileSync(cPath, "utf8"), firstC);
  assert.equal(readFileSync(runtimeRustPath, "utf8"), firstRuntimeRust);
  assert.equal(readFileSync(runtimeCppPath, "utf8"), firstRuntimeCpp);
  assert.match(firstC, /AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION/);
  assert.match(firstC, /AIVS_ERROR_INVALID_ARGUMENT/);
  assert.match(firstRuntimeRust, /pub const HEADER_SIZE: usize = 32/);
  assert.match(firstRuntimeRust, /pub const MAX_MESSAGES_PER_FEED: usize = 64/);
  assert.match(firstRuntimeRust, /pub const POLICIES:/);
  assert.match(firstRuntimeCpp, /kMaxControlPayloadBytes = 65536U/);
  assert.match(firstRuntimeCpp, /kMaxInputBytesPerFeed = 65568U/);
  assert.match(firstRuntimeCpp, /kPolicies/);
});

test("check rejects malformed RuntimeMessage layout and limits", (t) => {
  const overlapping = validRuntimeMessage();
  overlapping.encoding.header_fields[1].offset = 0;
  const overlappingRoot = fixture(t, { runtimeMessage: overlapping });
  assert.match(runFailure(overlappingRoot, "check"), /header fields must be contiguous/);

  const wrongFrameSize = validRuntimeMessage();
  wrongFrameSize.limits.max_frame_bytes = 65567;
  const wrongFrameSizeRoot = fixture(t, { runtimeMessage: wrongFrameSize });
  assert.match(runFailure(wrongFrameSizeRoot, "check"), /max_frame_bytes/);

  const unboundedBatch = validRuntimeMessage();
  delete unboundedBatch.limits.max_messages_per_feed;
  const unboundedBatchRoot = fixture(t, { runtimeMessage: unboundedBatch });
  assert.match(runFailure(unboundedBatchRoot, "generate"), /max_messages_per_feed/);

  const duplicateCommand = validRuntimeMessage();
  duplicateCommand.commands[2].value = 1;
  const duplicateCommandRoot = fixture(t, { runtimeMessage: duplicateCommand });
  assert.match(runFailure(duplicateCommandRoot, "check"), /duplicate command value/);

  for (const [collection, length] of [
    ["message_kinds", 3],
    ["commands", 5],
  ]) {
    for (let index = 0; index < length; index += 1) {
      const renumbered = validRuntimeMessage();
      renumbered[collection][index].value = 100 + index;
      const renumberedRoot = fixture(t, { runtimeMessage: renumbered });
      assert.match(runFailure(renumberedRoot, "generate"), /v1 numeric assignment/);
    }
  }

  const incompletePolicy = validRuntimeMessage();
  incompletePolicy.policies.pop();
  const incompletePolicyRoot = fixture(t, { runtimeMessage: incompletePolicy });
  assert.match(runFailure(incompletePolicyRoot, "generate"), /policy matrix/);

  const wrongFatalErrors = validRuntimeMessage();
  wrongFatalErrors.decoder.fatal_errors = ["InvalidState"];
  const wrongFatalErrorsRoot = fixture(t, { runtimeMessage: wrongFatalErrors });
  assert.match(runFailure(wrongFatalErrorsRoot, "generate"), /fatal_errors/);

  const unstableAllocationFailure = validRuntimeMessage();
  unstableAllocationFailure.decoder.allocation_failure.error_code = "MalformedFrame";
  const unstableAllocationFailureRoot = fixture(t, { runtimeMessage: unstableAllocationFailure });
  assert.match(runFailure(unstableAllocationFailureRoot, "generate"), /allocation_failure/);
});

test("check detects drift in every generated protocol artifact without rewriting it", (t) => {
  const root = fixture(t);
  run(root, "generate");
  const generatedPaths = [
    "core/contracts/src/generated.rs",
    "core/contracts/src/runtime_message_generated.rs",
    "core/contracts/include/ai_voice_contracts/generated_contracts.hpp",
    "core/contracts/include/ai_voice_contracts/generated_contracts_c.h",
    "core/contracts/include/ai_voice_contracts/runtime_message_generated.hpp",
  ].map((relativePath) => path.join(root, relativePath));

  for (const generatedPath of generatedPaths) {
    const current = readFileSync(generatedPath, "utf8");
    writeFileSync(generatedPath, "stale mapping\n");
    assert.match(runFailure(root, "check"), /generated mapping drift/);
    assert.equal(readFileSync(generatedPath, "utf8"), "stale mapping\n");
    writeFileSync(generatedPath, current);
  }
});

test("preserves frozen v1 mappings after active compatibility advances", (t) => {
  const version = validVersion();
  version.voice_engine_abi.current_version = 2;
  version.voice_engine_abi.minimum_compatible_version = 2;
  const root = fixture(t, { version });

  run(root, "generate");
  const cPath = path.join(root, "core/contracts/include/ai_voice_contracts/generated_contracts_c.h");
  assert.match(readFileSync(cPath, "utf8"), /AIVS_VOICE_ENGINE_ABI_V1_VERSION UINT32_C\(1\)/);
  assert.doesNotThrow(() => run(root, "check"));
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
