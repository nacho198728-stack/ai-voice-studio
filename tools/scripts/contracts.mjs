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
const RUNTIME_MESSAGE_KINDS = ["Hello", "Request", "Response"];
const RUNTIME_MESSAGE_COMMANDS = [
  "None",
  "Ping",
  "GetCapabilities",
  "RunMockPipeline",
  "Shutdown",
];
const SOURCE_DESCRIPTION =
  "core/contracts/version.json, core/contracts/error-codes.json, and core/contracts/runtime-message-v1.json";

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
    const keys = [
      "current_version",
      "minimum_compatible_version",
    ];
    if (sectionName === "voice_engine_abi") {
      keys.push("v1_version");
    }
    requireExactKeys(section, `version.json ${sectionName}`, keys);
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
    const range = { current, minimum };
    if (sectionName === "voice_engine_abi") {
      const v1 = requireInteger(section.v1_version, "voice_engine_abi v1 version", { min: 1 });
      if (v1 > current) {
        fail("voice_engine_abi v1 version must not exceed current version");
      }
      range.v1 = v1;
    }
    return range;
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

function validateRuntimeMessageDocument(runtimeMessage) {
  requireExactKeys(runtimeMessage, "runtime-message-v1.json", [
    "schema_version",
    "contract",
    "encoding",
    "protocol_version_source",
    "error_code_source",
    "limits",
    "message_kinds",
    "commands",
    "policies",
    "invariants",
    "payload_semantics",
    "decoder",
    "transport",
  ]);
  if (runtimeMessage.schema_version !== 1 || runtimeMessage.contract !== "RuntimeMessage") {
    fail("runtime-message-v1.json must define RuntimeMessage schema_version 1");
  }
  if (runtimeMessage.protocol_version_source !== "version.json#/ipc_protocol") {
    fail("RuntimeMessage protocol_version_source must reference version.json#/ipc_protocol");
  }
  if (runtimeMessage.error_code_source !== "error-codes.json#/codes") {
    fail("RuntimeMessage error_code_source must reference error-codes.json#/codes");
  }

  const encoding = runtimeMessage.encoding;
  requireExactKeys(encoding, "RuntimeMessage encoding", [
    "byte_order",
    "magic_ascii",
    "wire_version",
    "header_size_bytes",
    "header_fields",
    "flags_known_mask",
    "reserved_value",
  ]);
  if (encoding.byte_order !== "little-endian") {
    fail("RuntimeMessage byte_order must be little-endian");
  }
  const magic = requireString(encoding.magic_ascii, "RuntimeMessage magic_ascii");
  if (!/^[\x20-\x7e]{4}$/.test(magic)) {
    fail("RuntimeMessage magic_ascii must contain exactly four ASCII bytes");
  }
  const wireVersion = requireInteger(encoding.wire_version, "RuntimeMessage wire_version", {
    min: 1,
    max: 0xffff,
  });
  const headerSize = requireInteger(encoding.header_size_bytes, "RuntimeMessage header_size_bytes", {
    min: 1,
  });
  if (encoding.flags_known_mask !== 0 || encoding.reserved_value !== 0) {
    fail("RuntimeMessage v1 flags_known_mask and reserved_value must be zero");
  }
  const expectedFields = [
    ["magic", 0, 4, "ascii[4]"],
    ["wire_version", 4, 2, "u16"],
    ["message_kind", 6, 1, "u8"],
    ["flags", 7, 1, "u8"],
    ["protocol_version", 8, 4, "u32"],
    ["request_id", 12, 8, "u64"],
    ["command", 20, 2, "u16"],
    ["reserved", 22, 2, "u16"],
    ["error_code", 24, 4, "i32"],
    ["payload_length", 28, 4, "u32"],
  ];
  if (!Array.isArray(encoding.header_fields) || encoding.header_fields.length !== expectedFields.length) {
    fail("RuntimeMessage header_fields must define the canonical v1 header");
  }
  let nextOffset = 0;
  for (let index = 0; index < expectedFields.length; index += 1) {
    const field = encoding.header_fields[index];
    requireExactKeys(field, `RuntimeMessage header_fields[${index}]`, [
      "name",
      "offset",
      "width_bytes",
      "type",
    ]);
    const [name, offset, width, type] = expectedFields[index];
    if (
      field.name !== name ||
      field.offset !== offset ||
      field.width_bytes !== width ||
      field.type !== type ||
      field.offset !== nextOffset
    ) {
      fail("RuntimeMessage header fields must be contiguous and match the canonical v1 layout");
    }
    nextOffset += width;
  }
  if (nextOffset !== headerSize) {
    fail("RuntimeMessage header_size_bytes must equal the fixed field layout");
  }

  requireExactKeys(runtimeMessage.limits, "RuntimeMessage limits", [
    "max_control_payload_bytes",
    "max_frame_bytes",
    "max_input_bytes_per_feed",
    "max_messages_per_feed",
    "max_hello_payload_bytes",
    "max_error_payload_bytes",
    "max_ping_payload_bytes",
  ]);
  const maxPayload = requireInteger(
    runtimeMessage.limits.max_control_payload_bytes,
    "RuntimeMessage max_control_payload_bytes",
    { min: 1, max: 1024 * 1024 },
  );
  const maxFrame = requireInteger(runtimeMessage.limits.max_frame_bytes, "RuntimeMessage max_frame_bytes", {
    min: headerSize,
  });
  if (maxFrame !== headerSize + maxPayload) {
    fail("RuntimeMessage max_frame_bytes must equal header_size_bytes plus max_control_payload_bytes");
  }
  const maxInputPerFeed = requireInteger(
    runtimeMessage.limits.max_input_bytes_per_feed,
    "RuntimeMessage max_input_bytes_per_feed",
    { min: headerSize, max: 1024 * 1024 },
  );
  if (maxInputPerFeed !== maxFrame) {
    fail("RuntimeMessage max_input_bytes_per_feed must equal max_frame_bytes in v1");
  }
  const maxMessagesPerFeed = requireInteger(
    runtimeMessage.limits.max_messages_per_feed,
    "RuntimeMessage max_messages_per_feed",
    { min: 1, max: 1024 },
  );
  const boundedLimit = (name) => {
    const value = requireInteger(runtimeMessage.limits[name], `RuntimeMessage ${name}`);
    if (value > maxPayload) {
      fail(`RuntimeMessage ${name} must not exceed max_control_payload_bytes`);
    }
    return value;
  };
  const maxHello = boundedLimit("max_hello_payload_bytes");
  const maxError = boundedLimit("max_error_payload_bytes");
  const maxPing = boundedLimit("max_ping_payload_bytes");

  const validateEnum = (items, expectedNames, label, max) => {
    if (!Array.isArray(items) || items.length !== expectedNames.length) {
      fail(`RuntimeMessage ${label} must contain the canonical v1 set`);
    }
    const values = new Set();
    return items.map((item, index) => {
      requireExactKeys(item, `RuntimeMessage ${label}[${index}]`, ["name", "value"]);
      if (item.name !== expectedNames[index]) {
        fail(`RuntimeMessage ${label} must use canonical v1 order`);
      }
      const value = requireInteger(item.value, `RuntimeMessage ${label} ${item.name}`, { max });
      if (values.has(value)) {
        fail(`duplicate ${label === "commands" ? "command" : "message kind"} value ${value}`);
      }
      values.add(value);
      return { name: item.name, value };
    });
  };

  const kinds = validateEnum(runtimeMessage.message_kinds, RUNTIME_MESSAGE_KINDS, "message_kinds", 0xff);
  const commands = validateEnum(runtimeMessage.commands, RUNTIME_MESSAGE_COMMANDS, "commands", 0xffff);
  const expectedKindValues = [1, 2, 3];
  const expectedCommandValues = [0, 1, 2, 3, 4];
  if (
    kinds.some((item, index) => item.value !== expectedKindValues[index]) ||
    commands.some((item, index) => item.value !== expectedCommandValues[index])
  ) {
    fail("RuntimeMessage v1 numeric assignment is frozen; define a new wire version to renumber it");
  }

  const expectedPolicies = [
    ["Hello", "None", "zero", "success", "max_hello_payload_bytes"],
    ["Request", "Ping", "nonzero", "success", "max_ping_payload_bytes"],
    ["Request", "GetCapabilities", "nonzero", "success", "zero"],
    ["Request", "RunMockPipeline", "nonzero", "success", "max_control_payload_bytes"],
    ["Request", "Shutdown", "nonzero", "success", "zero"],
    ["Response", "Ping", "nonzero", "success", "max_ping_payload_bytes"],
    ["Response", "GetCapabilities", "nonzero", "success", "max_control_payload_bytes"],
    ["Response", "RunMockPipeline", "nonzero", "success", "max_control_payload_bytes"],
    ["Response", "Shutdown", "nonzero", "success", "zero"],
    ["Response", "Ping", "nonzero", "non_success", "max_error_payload_bytes"],
    ["Response", "GetCapabilities", "nonzero", "non_success", "max_error_payload_bytes"],
    ["Response", "RunMockPipeline", "nonzero", "non_success", "max_error_payload_bytes"],
    ["Response", "Shutdown", "nonzero", "non_success", "max_error_payload_bytes"],
  ];
  if (!Array.isArray(runtimeMessage.policies) || runtimeMessage.policies.length !== expectedPolicies.length) {
    fail("RuntimeMessage v1 policy matrix must contain every canonical kind/command/error rule");
  }
  const limitValues = {
    zero: 0,
    max_hello_payload_bytes: maxHello,
    max_error_payload_bytes: maxError,
    max_ping_payload_bytes: maxPing,
    max_control_payload_bytes: maxPayload,
  };
  const policies = runtimeMessage.policies.map((policy, index) => {
    requireExactKeys(policy, `RuntimeMessage policies[${index}]`, [
      "kind",
      "command",
      "request_id_rule",
      "error_rule",
      "payload_limit",
    ]);
    const expected = expectedPolicies[index];
    const actual = [
      policy.kind,
      policy.command,
      policy.request_id_rule,
      policy.error_rule,
      policy.payload_limit,
    ];
    if (actual.some((value, fieldIndex) => value !== expected[fieldIndex])) {
      fail("RuntimeMessage v1 policy matrix must match every canonical kind/command/error rule");
    }
    return {
      kind: policy.kind,
      command: policy.command,
      requestIdRule: policy.request_id_rule,
      errorRule: policy.error_rule,
      maxPayload: limitValues[policy.payload_limit],
    };
  });

  const semanticSections = [
    ["invariants", runtimeMessage.invariants, ["hello", "request", "response", "success_response", "error_response", "unknown_values"]],
    ["payload_semantics", runtimeMessage.payload_semantics, ["encoding", "hello", "ping", "get_capabilities", "run_mock_pipeline", "shutdown", "error"]],
    ["decoder", runtimeMessage.decoder, ["fatal_errors", "allocation_failure", "failed_state", "feed_atomicity", "per_feed_limits", "retention", "eof"]],
    ["transport", runtimeMessage.transport, ["neutral", "stdout", "diagnostics"]],
  ];
  for (const [label, section, keys] of semanticSections) {
    requireExactKeys(section, `RuntimeMessage ${label}`, keys);
    for (const key of keys) {
      if (label === "decoder" && (key === "fatal_errors" || key === "allocation_failure")) {
        continue;
      }
      if (label === "transport" && key === "neutral") {
        continue;
      }
      requireString(section[key], `RuntimeMessage ${label}.${key}`);
    }
  }
  const expectedFatalErrors = [
    "MalformedFrame",
    "FrameTooLarge",
    "UnsupportedProtocolVersion",
  ];
  if (
    !Array.isArray(runtimeMessage.decoder.fatal_errors) ||
    runtimeMessage.decoder.fatal_errors.length !== expectedFatalErrors.length ||
    runtimeMessage.decoder.fatal_errors.some(
      (name, index) => name !== expectedFatalErrors[index],
    )
  ) {
    fail("RuntimeMessage decoder fatal_errors must contain the canonical framing error set");
  }
  requireExactKeys(runtimeMessage.decoder.allocation_failure, "RuntimeMessage allocation_failure", [
    "scope",
    "error_code",
    "discard_current_input",
    "release_partial_and_output_capacity",
    "reset_required",
  ]);
  const allocationFailure = runtimeMessage.decoder.allocation_failure;
  if (
    allocationFailure.scope !== "C++ decoder allocation" ||
    allocationFailure.error_code !== "InternalError" ||
    allocationFailure.discard_current_input !== true ||
    allocationFailure.release_partial_and_output_capacity !== true ||
    allocationFailure.reset_required !== true
  ) {
    fail("RuntimeMessage allocation_failure must use the canonical terminal InternalError policy");
  }
  if (runtimeMessage.transport.neutral !== true) {
    fail("RuntimeMessage transport must remain neutral");
  }

  return {
    schemaVersion: runtimeMessage.schema_version,
    magic,
    wireVersion,
    headerSize,
    maxPayload,
    maxFrame,
    maxInputPerFeed,
    maxMessagesPerFeed,
    maxHello,
    maxError,
    maxPing,
    kinds,
    commands,
    policies,
  };
}

export function validateContracts({ version, errorCodes, runtimeMessage, rootVersion }) {
  const normalizedRootVersion = requireString(rootVersion, "root VERSION");
  if (!/^\d+\.\d+\.\d+$/.test(normalizedRootVersion)) {
    fail("root VERSION must use major.minor.patch");
  }
  const validatedVersion = validateVersionDocument(version, normalizedRootVersion);
  return {
    version: validatedVersion,
    errors: validateErrorCodeDocument(errorCodes),
    runtimeMessage: validateRuntimeMessageDocument(runtimeMessage),
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
  ).replace(
    `pub const VOICE_ENGINE_ABI_CURRENT_VERSION: u32 = ${model.version.voiceEngineAbi.current};`,
    `pub const VOICE_ENGINE_ABI_V1_VERSION: u32 = ${model.version.voiceEngineAbi.v1};\npub const VOICE_ENGINE_ABI_CURRENT_VERSION: u32 = ${model.version.voiceEngineAbi.current};`,
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
  return `// @generated by tools/scripts/contracts.mjs from ${SOURCE_DESCRIPTION}.\n// Do not edit manually; run \`pnpm contracts:generate\`.\n\n#pragma once\n\n#include <cstdint>\n#include <optional>\n#include <string_view>\n\nnamespace ai_voice::contracts {\n\ninline constexpr std::uint32_t kContractSchemaVersion = ${version.schemaVersion}U;\ninline constexpr std::uint32_t kErrorCodeSchemaVersion = ${errors.schemaVersion}U;\ninline constexpr std::string_view kRuntimeVersion = ${cppString(version.runtimeVersion)};\ninline constexpr std::uint32_t kIpcProtocolCurrentVersion = ${version.ipcProtocol.current}U;\ninline constexpr std::uint32_t kIpcProtocolMinimumCompatibleVersion = ${version.ipcProtocol.minimum}U;\ninline constexpr std::uint32_t kVoiceEngineAbiCurrentVersion = ${version.voiceEngineAbi.current}U;\ninline constexpr std::uint32_t kVoiceEngineAbiMinimumCompatibleVersion = ${version.voiceEngineAbi.minimum}U;\n\nconstexpr bool is_ipc_protocol_compatible(std::uint32_t version) {\n  return version >= kIpcProtocolMinimumCompatibleVersion && version <= kIpcProtocolCurrentVersion;\n}\n\nconstexpr bool is_voice_engine_abi_compatible(std::uint32_t version) {\n  return version >= kVoiceEngineAbiMinimumCompatibleVersion && version <= kVoiceEngineAbiCurrentVersion;\n}\n\nenum class ErrorCode : std::int32_t {\n${enumValues}\n};\n\nenum class ErrorCategory {\n${categoryValues}\n};\n\nconstexpr std::int32_t error_code_value(ErrorCode code) {\n  return static_cast<std::int32_t>(code);\n}\n\nconstexpr std::optional<ErrorCode> error_code_from_value(std::int32_t value) {\n  switch (value) {\n${valueMatches}\n    default: return std::nullopt;\n  }\n}\n\nconstexpr ErrorCategory error_code_category(ErrorCode code) {\n  switch (code) {\n${categoryMatches}\n  }\n  return ErrorCategory::Internal;\n}\n\n}  // namespace ai_voice::contracts\n`.replace(
    `inline constexpr std::uint32_t kVoiceEngineAbiCurrentVersion = ${version.voiceEngineAbi.current}U;`,
    `inline constexpr std::uint32_t kVoiceEngineAbiV1Version = ${version.voiceEngineAbi.v1}U;\ninline constexpr std::uint32_t kVoiceEngineAbiCurrentVersion = ${version.voiceEngineAbi.current}U;`,
  );
}

export function renderC(model) {
  const { version, errors } = model;
  const errorDefinitions = errors.codes
    .map((code) => `#define AIVS_ERROR_${cSymbol(code.name)} INT32_C(${code.value})`)
    .join("\n");
  return `// @generated by tools/scripts/contracts.mjs from ${SOURCE_DESCRIPTION}.\n// Do not edit manually; run \`pnpm contracts:generate\`.\n\n#ifndef AIVS_CONTRACTS_GENERATED_CONTRACTS_C_H\n#define AIVS_CONTRACTS_GENERATED_CONTRACTS_C_H\n\n#include <stdint.h>\n\n#define AIVS_CONTRACT_SCHEMA_VERSION UINT32_C(${version.schemaVersion})\n#define AIVS_ERROR_CODE_SCHEMA_VERSION UINT32_C(${errors.schemaVersion})\n#define AIVS_IPC_PROTOCOL_CURRENT_VERSION UINT32_C(${version.ipcProtocol.current})\n#define AIVS_IPC_PROTOCOL_MINIMUM_COMPATIBLE_VERSION UINT32_C(${version.ipcProtocol.minimum})\n#define AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION UINT32_C(${version.voiceEngineAbi.current})\n#define AIVS_VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION UINT32_C(${version.voiceEngineAbi.minimum})\n\ntypedef int32_t aivs_error_code_t;\n\n${errorDefinitions}\n\nstatic inline uint32_t aivs_is_ipc_protocol_compatible(uint32_t version) {\n  return version >= AIVS_IPC_PROTOCOL_MINIMUM_COMPATIBLE_VERSION\n      && version <= AIVS_IPC_PROTOCOL_CURRENT_VERSION;\n}\n\nstatic inline uint32_t aivs_is_voice_engine_abi_compatible(uint32_t version) {\n  return version >= AIVS_VOICE_ENGINE_ABI_MINIMUM_COMPATIBLE_VERSION\n      && version <= AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION;\n}\n\n#endif  // AIVS_CONTRACTS_GENERATED_CONTRACTS_C_H\n`.replace(
    `#define AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION UINT32_C(${version.voiceEngineAbi.current})`,
    `#define AIVS_VOICE_ENGINE_ABI_V1_VERSION UINT32_C(${version.voiceEngineAbi.v1})\n#define AIVS_VOICE_ENGINE_ABI_CURRENT_VERSION UINT32_C(${version.voiceEngineAbi.current})`,
  );
}

function renderRuntimeMessageRustBase(model) {
  const message = model.runtimeMessage;
  const kinds = message.kinds.map((item) => `    ${item.name} = ${item.value},`).join("\n");
  const kindMatches = message.kinds
    .map((item) => `        ${item.value} => Some(MessageKind::${item.name}),`)
    .join("\n");
  const commands = message.commands.map((item) => `    ${item.name} = ${item.value},`).join("\n");
  const commandMatches = message.commands
    .map((item) => `        ${item.value} => Some(Command::${item.name}),`)
    .join("\n");
  const policies = message.policies
    .map(
      (policy) =>
        `    Policy {\n        kind: MessageKind::${policy.kind},\n        command: Command::${policy.command},\n        request_id_rule: RequestIdRule::${policy.requestIdRule === "zero" ? "Zero" : "NonZero"},\n        error_rule: ErrorRule::${policy.errorRule === "success" ? "Success" : "NonSuccess"},\n        max_payload_bytes: ${policy.maxPayload},\n    },`,
    )
    .join("\n");
  return `// @generated by tools/scripts/contracts.mjs from ${SOURCE_DESCRIPTION}.\n// Do not edit manually; run \`pnpm contracts:generate\`.\n\npub const RUNTIME_MESSAGE_SCHEMA_VERSION: u32 = ${message.schemaVersion};\npub const MAGIC: [u8; 4] = *b${JSON.stringify(message.magic)};\npub const WIRE_VERSION: u16 = ${message.wireVersion};\npub const HEADER_SIZE: usize = ${message.headerSize};\npub const MAX_CONTROL_PAYLOAD_BYTES: usize = ${message.maxPayload};\npub const MAX_FRAME_BYTES: usize = ${message.maxFrame};\npub const MAX_HELLO_PAYLOAD_BYTES: usize = ${message.maxHello};\npub const MAX_ERROR_PAYLOAD_BYTES: usize = ${message.maxError};\npub const MAX_PING_PAYLOAD_BYTES: usize = ${message.maxPing};\n\n#[repr(u8)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum MessageKind {\n${kinds}\n}\n\npub const fn message_kind_from_value(value: u8) -> Option<MessageKind> {\n    match value {\n${kindMatches}\n        _ => None,\n    }\n}\n\n#[repr(u16)]\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum Command {\n${commands}\n}\n\npub const fn command_from_value(value: u16) -> Option<Command> {\n    match value {\n${commandMatches}\n        _ => None,\n    }\n}\n\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum RequestIdRule {\n    Zero,\n    NonZero,\n}\n\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub enum ErrorRule {\n    Success,\n    NonSuccess,\n}\n\n#[derive(Clone, Copy, Debug, Eq, PartialEq)]\npub struct Policy {\n    pub kind: MessageKind,\n    pub command: Command,\n    pub request_id_rule: RequestIdRule,\n    pub error_rule: ErrorRule,\n    pub max_payload_bytes: usize,\n}\n\npub const POLICIES: [Policy; ${message.policies.length}] = [\n${policies}\n];\n`;
}

export function renderRuntimeMessageRust(model) {
  const message = model.runtimeMessage;
  return renderRuntimeMessageRustBase(model).replace(
    `pub const MAX_FRAME_BYTES: usize = ${message.maxFrame};`,
    `pub const MAX_FRAME_BYTES: usize = ${message.maxFrame};\npub const MAX_INPUT_BYTES_PER_FEED: usize = ${message.maxInputPerFeed};\npub const MAX_MESSAGES_PER_FEED: usize = ${message.maxMessagesPerFeed};`,
  );
}

function renderRuntimeMessageCppBase(model) {
  const message = model.runtimeMessage;
  const magic = [...message.magic].map((byte) => `'${byte}'`).join(", ");
  const kinds = message.kinds.map((item) => `  ${item.name} = ${item.value},`).join("\n");
  const kindMatches = message.kinds
    .map((item) => `    case ${item.value}: return MessageKind::${item.name};`)
    .join("\n");
  const commands = message.commands.map((item) => `  ${item.name} = ${item.value},`).join("\n");
  const commandMatches = message.commands
    .map((item) => `    case ${item.value}: return Command::${item.name};`)
    .join("\n");
  const policies = message.policies
    .map(
      (policy) =>
        `    Policy{MessageKind::${policy.kind}, Command::${policy.command}, RequestIdRule::${policy.requestIdRule === "zero" ? "Zero" : "NonZero"}, ErrorRule::${policy.errorRule === "success" ? "Success" : "NonSuccess"}, ${policy.maxPayload}U},`,
    )
    .join("\n");
  return `// @generated by tools/scripts/contracts.mjs from ${SOURCE_DESCRIPTION}.\n// Do not edit manually; run \`pnpm contracts:generate\`.\n\n#pragma once\n\n#include <array>\n#include <cstddef>\n#include <cstdint>\n#include <optional>\n\nnamespace ai_voice::contracts::runtime_message {\n\ninline constexpr std::uint32_t kRuntimeMessageSchemaVersion = ${message.schemaVersion}U;\ninline constexpr std::array<std::uint8_t, 4> kMagic{${magic}};\ninline constexpr std::uint16_t kWireVersion = ${message.wireVersion}U;\ninline constexpr std::size_t kHeaderSize = ${message.headerSize}U;\ninline constexpr std::size_t kMaxControlPayloadBytes = ${message.maxPayload}U;\ninline constexpr std::size_t kMaxFrameBytes = ${message.maxFrame}U;\ninline constexpr std::size_t kMaxHelloPayloadBytes = ${message.maxHello}U;\ninline constexpr std::size_t kMaxErrorPayloadBytes = ${message.maxError}U;\ninline constexpr std::size_t kMaxPingPayloadBytes = ${message.maxPing}U;\n\nenum class MessageKind : std::uint8_t {\n${kinds}\n};\n\nconstexpr std::optional<MessageKind> message_kind_from_value(std::uint8_t value) {\n  switch (value) {\n${kindMatches}\n    default: return std::nullopt;\n  }\n}\n\nenum class Command : std::uint16_t {\n${commands}\n};\n\nconstexpr std::optional<Command> command_from_value(std::uint16_t value) {\n  switch (value) {\n${commandMatches}\n    default: return std::nullopt;\n  }\n}\n\nenum class RequestIdRule { Zero, NonZero };\nenum class ErrorRule { Success, NonSuccess };\n\nstruct Policy {\n  MessageKind kind;\n  Command command;\n  RequestIdRule request_id_rule;\n  ErrorRule error_rule;\n  std::size_t max_payload_bytes;\n};\n\ninline constexpr std::array<Policy, ${message.policies.length}> kPolicies{{\n${policies}\n}};\n\n}  // namespace ai_voice::contracts::runtime_message\n`;
}

export function renderRuntimeMessageCpp(model) {
  const message = model.runtimeMessage;
  return renderRuntimeMessageCppBase(model).replace(
    `inline constexpr std::size_t kMaxFrameBytes = ${message.maxFrame}U;`,
    `inline constexpr std::size_t kMaxFrameBytes = ${message.maxFrame}U;\ninline constexpr std::size_t kMaxInputBytesPerFeed = ${message.maxInputPerFeed}U;\ninline constexpr std::size_t kMaxMessagesPerFeed = ${message.maxMessagesPerFeed}U;`,
  );
}

function pathsFor(root) {
  const contractsRoot = path.join(root, "core", "contracts");
  return {
    version: path.join(contractsRoot, "version.json"),
    errorCodes: path.join(contractsRoot, "error-codes.json"),
    runtimeMessage: path.join(contractsRoot, "runtime-message-v1.json"),
    rootVersion: path.join(root, "VERSION"),
    rust: path.join(contractsRoot, "src", "generated.rs"),
    cpp: path.join(contractsRoot, "include", "ai_voice_contracts", "generated_contracts.hpp"),
    c: path.join(contractsRoot, "include", "ai_voice_contracts", "generated_contracts_c.h"),
    runtimeRust: path.join(contractsRoot, "src", "runtime_message_generated.rs"),
    runtimeCpp: path.join(
      contractsRoot,
      "include",
      "ai_voice_contracts",
      "runtime_message_generated.hpp",
    ),
  };
}

function outputsFor(root) {
  const paths = pathsFor(root);
  const rootVersion = readFileSync(paths.rootVersion, "utf8").trim();
  const model = validateContracts({
    version: readJson(paths.version),
    errorCodes: readJson(paths.errorCodes),
    runtimeMessage: readJson(paths.runtimeMessage),
    rootVersion,
  });
  return [
    { path: paths.rust, contents: renderRust(model) },
    { path: paths.cpp, contents: renderCpp(model) },
    { path: paths.c, contents: renderC(model) },
    { path: paths.runtimeRust, contents: renderRuntimeMessageRust(model) },
    { path: paths.runtimeCpp, contents: renderRuntimeMessageCpp(model) },
  ];
}

function generatedTextMatches(actual, expected) {
  return actual.replaceAll("\r\n", "\n") === expected;
}

function generate(root) {
  for (const output of outputsFor(root)) {
    if (
      !existsSync(output.path)
      || !generatedTextMatches(readFileSync(output.path, "utf8"), output.contents)
    ) {
      mkdirSync(path.dirname(output.path), { recursive: true });
      writeFileSync(output.path, output.contents);
      process.stdout.write(`generated ${path.relative(root, output.path)}\n`);
    }
  }
}

function check(root) {
  for (const output of outputsFor(root)) {
    if (
      !existsSync(output.path)
      || !generatedTextMatches(readFileSync(output.path, "utf8"), output.contents)
    ) {
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
