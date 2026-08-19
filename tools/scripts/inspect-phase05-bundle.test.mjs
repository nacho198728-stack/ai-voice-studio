import assert from "node:assert/strict";
import test from "node:test";

import {
  expectedPhase05BundleFiles,
  validatePhase05BundleEvidence,
} from "./inspect-phase05-bundle.mjs";

const target = "aarch64-apple-darwin";

function passingEvidence() {
  const inventory = expectedPhase05BundleFiles(target).map((relativePath) => ({
    relativePath,
    bytes: 1,
    sha256: "0".repeat(64),
    fileType: relativePath.endsWith(".dylib")
      ? "Mach-O 64-bit dynamically linked shared library arm64"
      : relativePath.includes("/MacOS/")
        ? "Mach-O 64-bit executable arm64"
        : "data",
  }));
  return {
    schemaVersion: 1,
    target,
    bundleRoot: "/tmp/AI Voice Studio.app",
    inventory,
    binaries: [
      {
        relativePath: "Contents/MacOS/ai-voice-studio",
        architectures: ["arm64"],
        dependencies: [
          "/System/Library/Frameworks/WebKit.framework/Versions/A/WebKit",
          "/usr/lib/libSystem.B.dylib",
        ],
        undefinedSymbols: ["_NSApplicationMain"],
      },
      {
        relativePath: "Contents/MacOS/voice-runtime",
        architectures: ["arm64"],
        dependencies: ["/usr/lib/libc++.1.dylib", "/usr/lib/libSystem.B.dylib"],
        undefinedSymbols: ["___cxa_begin_catch"],
      },
      {
        relativePath:
          "Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib",
        architectures: ["arm64"],
        dependencies: [
          "@rpath/libaivs_mock_voice_engine.dylib",
          "/usr/lib/libc++.1.dylib",
          "/usr/lib/libSystem.B.dylib",
        ],
        undefinedSymbols: ["___cxa_begin_catch"],
      },
    ],
  };
}

test("accepts the exact arm64 Phase 0.5 app inventory and system-only linkage", () => {
  const result = validatePhase05BundleEvidence(passingEvidence());
  assert.deepEqual(result.exclusions, {
    bundledPythonOrAiRuntime: false,
    bundledModelOrAudioAsset: false,
    forbiddenDynamicDependency: false,
    forbiddenRuntimeSymbol: false,
    nonArm64MachO: false,
  });
});

test("rejects extra model, Python, archive, and audio asset package content", () => {
  for (const relativePath of [
    "Contents/Resources/model.onnx",
    "Contents/Resources/python/site.py",
    "Contents/Resources/libtorch.a",
    "Contents/Resources/sample.wav",
  ]) {
    const evidence = passingEvidence();
    evidence.inventory.push({
      relativePath,
      bytes: 1,
      sha256: "1".repeat(64),
      fileType: "data",
    });
    assert.throws(
      () => validatePhase05BundleEvidence(evidence),
      new RegExp(`unexpected bundle file.*${relativePath.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&")}`, "u"),
    );
  }
});

test("rejects Python, PyTorch, CUDA, ONNX, and audio-device dependencies", () => {
  for (const dependency of [
    "/opt/lib/libpython3.12.dylib",
    "/opt/lib/libtorch_cpu.dylib",
    "/opt/lib/libcuda.dylib",
    "/opt/lib/libonnxruntime.dylib",
    "/System/Library/Frameworks/CoreAudio.framework/Versions/A/CoreAudio",
  ]) {
    const evidence = passingEvidence();
    evidence.binaries[0].dependencies.push(dependency);
    assert.throws(() => validatePhase05BundleEvidence(evidence), /forbidden dynamic dependency/u);
  }
});

test("rejects direct AI-runtime and real audio-device API imports", () => {
  for (const symbol of [
    "_Py_Initialize",
    "_OrtGetApiBase",
    "_cudaMalloc",
    "_AudioObjectGetPropertyData",
    "_AudioUnitInitialize",
    "_Pa_OpenStream",
  ]) {
    const evidence = passingEvidence();
    evidence.binaries[1].undefinedSymbols.push(symbol);
    assert.throws(() => validatePhase05BundleEvidence(evidence), /forbidden runtime symbol/u);
  }
});

test("does not mistake a Rust configuration type name for an audio-device API", () => {
  const evidence = passingEvidence();
  evidence.binaries[0].globalSymbols = [
    "__RNvXNvXNvCsh6phXTN0fg7_15ai_voice_configs6_1__NtB7_20AudioDeviceSelection",
  ];
  assert.doesNotThrow(() => validatePhase05BundleEvidence(evidence));
});

test("rejects a missing required artifact, symlink, or non-arm64 Mach-O", () => {
  const missing = passingEvidence();
  missing.inventory.pop();
  assert.throws(() => validatePhase05BundleEvidence(missing), /bundle inventory mismatch/u);

  const symlink = passingEvidence();
  symlink.inventory[0].symlink = true;
  assert.throws(() => validatePhase05BundleEvidence(symlink), /symlink/u);

  const wrongArchitecture = passingEvidence();
  wrongArchitecture.binaries[0].architectures = ["x86_64"];
  assert.throws(() => validatePhase05BundleEvidence(wrongArchitecture), /arm64/u);
});
