#!/usr/bin/env node
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const COMPATIBILITY_RULE =
  "A peer is compatible if and only if its integer version is in the inclusive range [minimum_compatible_version, current_version].";
const CATEGORY_NAMES = ["success", "version", "framing", "runtime", "engine", "internal"];
const REQUIRED_ERROR_CODES = [
  "Success",
  "UnsupportedProtocolVersion",
  "UnsupportedVoiceEngineAbi",
  "MalformedFrame",
  "FrameTooLarge",
  "RuntimeUnavailable",
  "RuntimeShuttingDown",
  "EngineUnavailable",
  "InvalidArgument",
  "InvalidState",
  "BufferTooSmall",
  "InternalError",
];
const SOURCE_DESCRIPTION =
  "core/contracts/version.json and core/contracts/error-codes.json";

class ContractValidationError extends Error {}

function fail(message) {
  throw new ContractValidationError(`contracts: ${message}`);
}

function requireObject(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${label} must be an object`);
  }
  return value;
}

function requireExactKeys(value, label, keys) {
  const actual = Object.keys(requireObject(value, label)).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    fail(`${label} must contain exactly: ${expected.join(", ")}`);
  }
}

function requireInteger(value, label, { min = 0, max = 0xffffffff } = {}) {
  if (!Number.isInteger(value) || value < min || value > max) {
    fail(`${label} must be an integer in [${min}, ${max}]`);
  }
  return value;
}

function requireString(value, label) {
  if (typeof value !== "string" || value.length === 0 || /[\r\n]/.test(value)) {
    fail(`${label} must be a non-empty single-line string`);
  }
  return value;
}

function readJson(filePath) {
  let text;
  try {
    text = readFileSync(filePath, "utf8");
  } catch (error) {
    fail(`cannot read ${filePath}: ${error.message}`);
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    fail(`invalid JSON in ${filePath}: ${error.message}`);
  }
}

function validateVersionDocument(version, rootVersion) {
  requireExactKeys(version, "version.json", [
    "schema_version",
    "runtime",
    "ipc_protocol",
    "voice_engine_abi",
    "compatibility_rule",
  ]);
  if (version.schema_version !== 1) {
    fail("version.json schema_version must be 1");
  }
  requireExactKeys(version.runtime, "version.json runtime", ["version"]);
  const runtimeVersion = requireString(version.runtime.version, "Runtime version");
  if (!/^\d+\.\d+\.\d+$/.test(runtimeVersion)) {
    fail("Runtime version must use major.minor.patch");
  }
  if (runtimeVersion !== rootVersion) {
    fail(`Runtime version ${runtimeVersion} differs from root VERSION ${rootVersion}`);
  }
  if (version.compatibility_rule !== COMPATIBILITY_RULE) {
    fail("version.json compatibility_rule is not the canonical inclusive range rule");
  }

  const validateRange = (sectionName) => {
    const section = version[sectionName];
    requireExactKeys(section, `version.json ${sectionName}`, [
      "current_version",
      "minimum_compatible_version",
    ]);
    const current = requireInteger(section.current_version, `${sectionName} current version`, {
      min: 1,
    });
    const minimum = requireInteger(
      section.minimum_compatible_version,
      `${sectionName} minimum compatible version`,
      { min: 1 },
    );
    if (minimum > current) {
      fail(`${sectionName} minimum compatible version must not exceed current version`);
    }
    return { current, minimum };
  };

  return {
    schemaVersion: version.schema_version,
    runtimeVersion,
    ipcProtocol: validateRange("ipc_protocol"),
    voiceEngineAbi: validateRange("voice_engine_abi"),
  };
}

function validateErrorCodeDocument(errorCodes) {
  requireExactKeys(errorCodes, "error-codes.json", ["schema_version", "numeric_ranges", "codes"]);
  if (errorCodes.schema_version !== 1) {
    fail("error-codes.json schema_version must be 1");
  }
  if (!Array.isArray(errorCodes.numeric_ranges) || errorCodes.numeric_ranges.length !== CATEGORY_NAMES.length) {
    fail("error-codes.json numeric_ranges must define every reserved category once");
  }

  const ranges = [];
  let previousEnd = -1;
  for (let index = 0; index < errorCodes.numeric_ranges.length; index += 1) {
    const range = errorCodes.numeric_ranges[index];
    requireExactKeys(range, `numeric_ranges[${index}]`, ["name", "start", "end"]);
    if (range.name !== CATEGORY_NAMES[index]) {
      fail("numeric_ranges must use canonical category order");
    }
    const start = requireInteger(range.start, `${range.name} range start`, { max: 0x7fffffff });
    const end = requireInteger(range.end, `${range.name} range end`, { max: 0x7fffffff });
    if (start > end || start <= previousEnd) {
      fail("numeric_ranges must be non-overlapping and ordered");
    }
    previousEnd = end;
    ranges.push({ name: range.name, start, end });
  }

  if (!Array.isArray(errorCodes.codes) || errorCodes.codes.length !== REQUIRED_ERROR_CODES.length) {
    fail("error-codes.json codes must contain the Phase 0.5 boundary code set");
  }
  const names = new Set();
  const values = new Set();
  const codes = [];
  let previousValue = -1;
  for (let index = 0; index < errorCodes.codes.length; index += 1) {
    const code = errorCodes.codes[index];
    requireExactKeys(code, `codes[${index}]`, ["name", "value", "category", "meaning"]);
    const name = requireString(code.name, `codes[${index}].name`);
    if (!/^[A-Z][A-Za-z0-9]*$/.test(name)) {
      fail(`error code name ${name} is not a stable language symbol`);
    }
    if (names.has(name)) {
      fail(`duplicate error code name ${name}`);
    }
    if (name !== REQUIRED_ERROR_CODES[index]) {
      fail("error-codes.json codes must use canonical Phase 0.5 order");
    }
    names.add(name);
    const value = requireInteger(code.value, `error code ${name} value`, { max: 0x7fffffff });
    if (values.has(value)) {
      fail(`duplicate error code value ${value}`);
    }
    if (value <= previousValue) {
      fail("error-codes.json code values must be strictly ascending");
    }
    values.add(value);
    previousValue = value;
    const category = requireString(code.category, `error code ${name} category`);
    const range = ranges.find((candidate) => candidate.name === category);
    if (!range) {
      fail(`error code ${name} has an unknown category ${category}`);
    }
    if (value < range.start || value > range.end) {
      fail(`error code ${name} value is outside its ${category} reserved range`);
    }
    codes.push({
      name,
      value,
      category,
      meaning: requireString(code.meaning, `error code ${name} meaning`),
    });
  }
  return { schemaVersion: errorCodes.schema_version, ranges, codes };
}

export function validateContracts({ version, errorCodes, rootVersion }) {
  const normalizedRootVersion = requireString(rootVersion, "root VERSION");
  if (!/^\d+\.\d+\.\d+$/.test(normalizedRootVersion)) {
    fail("root VERSION must use major.minor.patch");
  }
  return {
    version: validateVersionDocument(version, normalizedRootVersion),
    errors: validateErrorCodeDocument(errorCodes),
  };
}

function rustCategory(category) {
  return category[0].toUpperCase() + category.slice(1);
}

function cppString(value) {
  return JSON.stringify(value).replaceAll("\\u2028", "\\u2028").replaceAll("\\u2029", "\\u2029");
}

function cSymbol(name) {
  return name.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toUpperCase();
}

function renderRustUnformatted(model) {
  const { version, errors } = model;
  const codeVariants = errors.codes.map((code) => `    ${code.name} = ${code.value},`).join("\n");
  const valueMatches = errors.codes
    .map((code) => `        ${code.value} => Some(ErrorCode::${code.name}),`)
    .join("\n");
  const categoryMatches = errors.codes
    .map((code) => `        ErrorCode::${code.name} => ErrorCategory::${rustCategory(code.category)},`)
    .join("\n");
  const categories = CATEGORY_NAMES.map((category) => `    ${rustCategory(category)},`).join("\n");
  return `// @generated by tools/scripts/contracts.mjs from ${SOURCE_DESCRIPTION}.\n// Do not edit manually; run \`pnpm contracts:generate\`.\n\npub const CONTRACT_SCHEMA_VERSION: u32 = ${version.schemaVersion};\npub const ERROR_CODE_SCHEMA_VERSION: u32 = ${errors.schemaVersion};\npub const RUNTIME_VERSION: &str = ${JSON.stringify(version.runtimeVersion)};\npub const IPC_PROTOCOL_CURRENT_VERSION: u32 = ${version.ipcProtocol.current};\npub const IPC_PROTOCOL_MINIMUM_COMPATIBLE_VERSION: u32 = ${version.ipcProtocol.minimum};\npub const VOICE_ENGINE_ABI_CURRENT_VERSION: u32 = ${version.voiceEngineAbi.current};\npub const VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION: u32 = ${version.voiceEngineAbi.minimum};\n\npub const fn is_ipc_protocol_compatible(version: u32) -> bool {\n    version >= IPC_PROTOCOL_MINIMUM_COMPATIBLE_VERSION && version <= IPC_PROTOCOL_CURRENT_VERSION\n}\n\npub const fn is_voice_engine_abi_compatible(version: u32) -> bool {\n    version >= VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION && version <= VOICE_ENGINE_ABI_CURRENT_VERSION\n}\n\n#[repr(i32)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum ErrorCode {\n${codeVariants}\n}\n\nimpl ErrorCode {\n    pub const fn value(self) -> i32 {\n        self as i32\n    }\n}\n\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum ErrorCategory {\n${categories}\n}\n\npub const fn error_code_from_value(value: i32) -> Option<ErrorCode> {\n    match value {\n${valueMatches}\n        _ => None,\n    }\n}\n\npub const fn error_code_category(code: ErrorCode) -> ErrorCategory {\n    match code {\n${categoryMatches}\n    }\n}\n`;
}

export function renderRust(model) {
  return renderRustUnformatted(model).replace(
    "    version >= VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION && version <= VOICE_ENGINE_ABI_CURRENT_VERSION",
    "    version >= VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION\n        && version <= VOICE_ENGINE_ABI_CURRENT_VERSION",
  );
}

export function renderCpp(model) {
  const { version, errors } = model;
  const enumValues = errors.codes.map((code) => `  ${code.name} = ${code.value},`).join("\n");
  const valueMatches = errors.codes
    .map((code) => `    case ${code.value}: return ErrorCode::${code.name};`)
    .join("\n");
  const categoryValues = CATEGORY_NAMES.map((category) => `  ${rustCategory(category)},`).join("\n");
  const categoryMatches = errors.codes
    .map((code) => `    case ErrorCode::${code.name}: return ErrorCategory::${rustCategory(code.category)};`)
    .join("\n");
  return `// @generated by tools/scripts/contracts.mjs from ${SOURCE_DESCRIPTION}.\n// Do not edit manually; run \`pnpm contracts:generate\`.\n\n#pragma once\n\n#include <cstdint>\n#include <optional>\n#include <string_view>\n\nnamespace ai_voice::contracts {\n\ninline constexpr std::uint32_t kContractSchemaVersion = ${version.schemaVersion}U;\ninline constexpr std::uint32_t kErrorCodeSchemaVersion = ${errors.schemaVersion}U;\ninline constexpr std::string_view kRuntimeVersion = ${cppString(version.runtimeVersion)};\ninline constexpr std::uint32_t kIpcProtocolCurrentVersion = ${version.ipcProtocol.current}U;\ninline constexpr std::uint32_t kIpcProtocolMinimumCompatibleVersion = ${version.ipcProtocol.minimum}U;\ninline constexpr std::uint32_t kVoiceEngineAbiCurrentVersion = ${version.voiceEngineAbi.current}U;\ninline constexpr std::uint32_t kVoiceEngineAbiMinimumCompatibleVersion = ${version.voiceEngineAbi.minimum}U;\n\nconstexpr bool is_ipc_protocol_compatible(std::uint32_t version) {\n  return version >= kIpcProtocolMinimumCompatibleVersion && version <= kIpcProtocolCurrentVersion;\n}\n\nconstexpr bool is_voice_engine_abi_compatible(std::uint32_t version) {\n  return version >= kVoiceEngineAbiMinimumCompatibleVersion && version <= kVoiceEngineAbiCurrentVersion;\n}\n\nenum class ErrorCode : std::int32_t {\n${enumValues}\n};\n\nenum class ErrorCategory {\n${categoryValues}\n};\n\nconstexpr std::int32_t error_code_value(ErrorCode code) {\n  return static_cast<std::int32_t>(code);\n}\n\nconstexpr std::optional<ErrorCode> error_code_from_value(std::int32_t value) {\n  switch (value) {\n${valueMatches}\n    default: return std::nullopt;\n  }\n}\n\nconstexpr ErrorCategory error_code_category(ErrorCode code) {\n  switch (code) {\n${categoryMatches}\n  }\n  return ErrorCategory::Internal;\n}\n\n}  // namespace ai_voice::contracts\n`;
}

export function renderC(model) {
  const { version, errors } = model;
  const errorDefinitions = errors.codes
    .map((code) => `#define AIVS_ERROR_${cSymbol(code.name)} INT32_C(${code.value})`)
    .join("\n");
  return `// @generated by tools/scripts/contracts.mjs from ${SOURCE_DESCRIPTION}.\n// Do not edit manually; run \`pnpm contracts:generate\`.\n\n#ifndef AIVS_CONTRACTS_GENERATED_CONTRACTS_C_H\n#define AIVS_CONTRACTS_GENERATED_CONTRACTS_C_H\n\n#include <stdint.h>\n\n#define AIVS_CONTRACT_SCHEMA_VERSION UINT32_C(${version.schemaVersion})\n#define AIVS_ERROR_CODE_SCHEMA_VERSION UINT32_C(${errors.schemaVersion})\n#define AIVS_IPC_PROTOCOL_CURRENT_VERSION UINT32_C(${version.ipcProtocol.current})\n#define AIVS_IPC_PROTOCOL_MINIMUM_COMPATIBLE_VERSION UINT32_C(${version.ipcProtocol.minimum})\n#define AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION UINT32_C(${version.voiceEngineAbi.current})\n#define AIVS_VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION UINT32_C(${version.voiceEngineAbi.minimum})\n\ntypedef int32_t aivs_error_code_t;\n\n${errorDefinitions}\n\nstatic inline uint32_t aivs_is_ipc_protocol_compatible(uint32_t version) {\n  return version >= AIVS_IPC_PROTOCOL_MINIMUM_COMPATIBLE_VERSION\n      && version <= AIVS_IPC_PROTOCOL_CURRENT_VERSION;\n}\n\nstatic inline uint32_t aivs_is_voice_engine_abi_compatible(uint32_t version) {\n  return version >= AIVS_VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION\n      && version <= AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION;\n}\n\n#endif  // AIVS_CONTRACTS_GENERATED_CONTRACTS_C_H\n`;
}

function pathsFor(root) {
  const contractsRoot = path.join(root, "core", "contracts");
  return {
    version: path.join(contractsRoot, "version.json"),
    errorCodes: path.join(contractsRoot, "error-codes.json"),
    rootVersion: path.join(root, "VERSION"),
    rust: path.join(contractsRoot, "src", "generated.rs"),
    cpp: path.join(contractsRoot, "include", "ai_voice_contracts", "generated_contracts.hpp"),
    c: path.join(contractsRoot, "include", "ai_voice_contracts", "generated_contracts_c.h"),
  };
}

function outputsFor(root) {
  const paths = pathsFor(root);
  const rootVersion = readFileSync(paths.rootVersion, "utf8").trim();
  const model = validateContracts({
    version: readJson(paths.version),
    errorCodes: readJson(paths.errorCodes),
    rootVersion,
  });
  return [
    { path: paths.rust, contents: renderRust(model) },
    { path: paths.cpp, contents: renderCpp(model) },
    { path: paths.c, contents: renderC(model) },
  ];
}

function generate(root) {
  for (const output of outputsFor(root)) {
    if (!existsSync(output.path) || readFileSync(output.path, "utf8") !== output.contents) {
      mkdirSync(path.dirname(output.path), { recursive: true });
      writeFileSync(output.path, output.contents);
      process.stdout.write(`generated ${path.relative(root, output.path)}\n`);
    }
  }
}

function check(root) {
  for (const output of outputsFor(root)) {
    if (!existsSync(output.path) || readFileSync(output.path, "utf8") !== output.contents) {
      fail(`generated mapping drift: ${path.relative(root, output.path)}; run pnpm contracts:generate`);
    }
  }
  process.stdout.write("contracts are valid and generated mappings are current\n");
}

function parseArguments(argv) {
  const [mode, ...rest] = argv;
  if (mode !== "generate" && mode !== "check") {
    fail("usage: contracts.mjs <generate|check> [--root <repository-root>]");
  }
  let root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
  for (let index = 0; index < rest.length; index += 1) {
    if (rest[index] !== "--root" || index + 1 >= rest.length || index + 2 !== rest.length) {
      fail("usage: contracts.mjs <generate|check> [--root <repository-root>]");
    }
    root = path.resolve(rest[index + 1]);
    index += 1;
  }
  return { mode, root };
}

function main() {
  try {
    const { mode, root } = parseArguments(process.argv.slice(2));
    if (mode === "generate") {
      generate(root);
    } else {
      check(root);
    }
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exitCode = 1;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main();
}
