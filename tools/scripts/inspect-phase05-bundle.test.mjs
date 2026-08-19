import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import * as bundleInspection from "./inspect-phase05-bundle.mjs";
import {
  collectMachOEvidence,
  expectedPhase05BundleFiles,
  validatePhase05BundleEvidence,
} from "./inspect-phase05-bundle.mjs";

const target = "aarch64-apple-darwin";
const bundleRoot = "/tmp/AI Voice Studio.app";
const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function inspectedBinary(
  relativePath,
  {
    architectures = ["arm64"],
    dependencies = [],
    undefinedSymbols = [],
    globalSymbols = [],
    failTool,
  } = {},
) {
  return collectMachOEvidence(bundleRoot, relativePath, (executable, toolArguments) => {
    const key = `${executable} ${toolArguments[0]}`;
    if (key === failTool) throw new Error(`controlled ${key} failure`);
    if (executable === "/usr/bin/lipo") return `${architectures.join(" ")}\n`;
    if (executable === "/usr/bin/otool") {
      return [
        `${toolArguments.at(-1)}:`,
        ...dependencies.map(
          (dependency) => `\t${dependency} (compatibility version 1.0.0, current version 1.0.0)`,
        ),
      ].join("\n");
    }
    const symbols = toolArguments[0] === "-u" ? undefinedSymbols : globalSymbols;
    return symbols.map((symbol) => `                 U ${symbol}`).join("\n");
  });
}

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
    bundleRoot,
    inventory,
    binaries: [
      inspectedBinary("Contents/MacOS/ai-voice-studio", {
        dependencies: [
          "/System/Library/Frameworks/WebKit.framework/Versions/A/WebKit",
          "/usr/lib/libSystem.B.dylib",
        ],
        undefinedSymbols: ["_NSApplicationMain"],
      }),
      inspectedBinary("Contents/MacOS/voice-runtime", {
        dependencies: ["/usr/lib/libc++.1.dylib", "/usr/lib/libSystem.B.dylib"],
        undefinedSymbols: ["___cxa_begin_catch"],
      }),
      inspectedBinary(
        "Contents/Resources/native/aivs_mock_voice_engine-aarch64-apple-darwin.dylib",
        {
        dependencies: [
          "@rpath/libaivs_mock_voice_engine.dylib",
          "/usr/lib/libc++.1.dylib",
          "/usr/lib/libSystem.B.dylib",
        ],
        undefinedSymbols: ["___cxa_begin_catch"],
        },
      ),
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
    evidence.binaries[0] = inspectedBinary("Contents/MacOS/ai-voice-studio", {
      dependencies: [
        "/System/Library/Frameworks/WebKit.framework/Versions/A/WebKit",
        "/usr/lib/libSystem.B.dylib",
        dependency,
      ],
      undefinedSymbols: ["_NSApplicationMain"],
    });
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
    evidence.binaries[1] = inspectedBinary("Contents/MacOS/voice-runtime", {
      dependencies: ["/usr/lib/libc++.1.dylib", "/usr/lib/libSystem.B.dylib"],
      undefinedSymbols: ["___cxa_begin_catch", symbol],
    });
    assert.throws(() => validatePhase05BundleEvidence(evidence), /forbidden runtime symbol/u);
  }
});

test("does not mistake a Rust configuration type name for an audio-device API", () => {
  const evidence = passingEvidence();
  evidence.binaries[0] = inspectedBinary("Contents/MacOS/ai-voice-studio", {
    dependencies: [
      "/System/Library/Frameworks/WebKit.framework/Versions/A/WebKit",
      "/usr/lib/libSystem.B.dylib",
    ],
    undefinedSymbols: ["_NSApplicationMain"],
    globalSymbols: [
      "__RNvXNvXNvCsh6phXTN0fg7_15ai_voice_configs6_1__NtB7_20AudioDeviceSelection",
    ],
  });
  assert.doesNotThrow(() => validatePhase05BundleEvidence(evidence));
});

test("rejects exact-looking Mach-O data when inspection provenance is missing", () => {
  const evidence = passingEvidence();
  delete evidence.binaries[0].inspections.globalSymbols;
  assert.throws(
    () => validatePhase05BundleEvidence(evidence),
    /completed Mach-O inspection evidence/u,
  );
});

test("accepts empty dependency and symbol results only after successful tools", () => {
  const evidence = passingEvidence();
  evidence.binaries[1] = inspectedBinary("Contents/MacOS/voice-runtime");
  assert.deepEqual(
    Object.fromEntries(
      Object.entries(evidence.binaries[1].inspections).map(([key, inspection]) => [
        key,
        [inspection.evidencePresent, inspection.completed],
      ]),
    ),
    {
      architectures: [true, true],
      dependencies: [true, true],
      undefinedSymbols: [true, true],
      globalSymbols: [true, true],
    },
  );
  assert.doesNotThrow(() => validatePhase05BundleEvidence(evidence));
});

test("rejects a tool failure and a forged completed observation", () => {
  const failed = passingEvidence();
  failed.binaries[1] = inspectedBinary("Contents/MacOS/voice-runtime", {
    failTool: "/usr/bin/nm -u",
  });
  assert.throws(
    () => validatePhase05BundleEvidence(failed),
    /completed Mach-O inspection evidence/u,
  );

  const forged = passingEvidence();
  forged.binaries[0].inspections.dependencies = {
    tool: "/usr/bin/otool",
    arguments: ["-L", `${bundleRoot}/Contents/MacOS/ai-voice-studio`],
    completed: true,
    output: `${bundleRoot}/Contents/MacOS/ai-voice-studio:\n`,
  };
  forged.binaries[0].dependencies = [];
  assert.throws(
    () => validatePhase05BundleEvidence(forged),
    /completed Mach-O inspection evidence/u,
  );
});

test("rejects a missing required artifact, symlink, or non-arm64 Mach-O", () => {
  const missing = passingEvidence();
  missing.inventory.pop();
  assert.throws(() => validatePhase05BundleEvidence(missing), /bundle inventory mismatch/u);

  const symlink = passingEvidence();
  symlink.inventory[0].symlink = true;
  assert.throws(() => validatePhase05BundleEvidence(symlink), /symlink/u);

  const wrongArchitecture = passingEvidence();
  wrongArchitecture.binaries[0] = inspectedBinary("Contents/MacOS/ai-voice-studio", {
    architectures: ["x86_64"],
    dependencies: ["/usr/lib/libSystem.B.dylib"],
  });
  assert.throws(() => validatePhase05BundleEvidence(wrongArchitecture), /arm64/u);
});

test("builds a deterministic manifest without paths, timestamps, or raw symbols", () => {
  assert.equal(typeof bundleInspection.buildPhase05AcceptanceManifest, "function");
  const manifest = bundleInspection.buildPhase05AcceptanceManifest(passingEvidence(), {
    product: { declared: "0.0.0", observed: "0.0.0", authority: "VERSION + Info.plist" },
    runtime: { declared: "0.0.0", observed: "0.0.0", authority: "version.json" },
    ipcProtocol: { declared: "1", observed: "1", authority: "version.json" },
    voiceEngineAbi: { declared: "1", observed: "1", authority: "version.json" },
    tools: {
      node: { declared: "24.16.0", observed: "24.16.0", authority: ".node-version" },
    },
  });
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.artifacts.length, 6);
  assert.deepEqual(manifest.symbolTotals, {
    undefinedTotal: 3,
    undefinedUnique: 2,
    globalTotal: 0,
    globalUnique: 0,
  });
  assert.deepEqual(manifest.binaries[0].inspections.dependencies, {
    tool: "/usr/bin/otool",
    arguments: ["-L"],
    evidencePresent: true,
    completed: true,
  });
  const serialized = JSON.stringify(manifest);
  assert.equal(serialized.includes(bundleRoot), false);
  assert.equal(serialized.includes("_NSApplicationMain"), false);
  assert.equal("generatedAt" in manifest, false);
});

test("reads every manifest version from repository authority files", async () => {
  assert.equal(typeof bundleInspection.readPhase05DeclaredVersions, "function");
  assert.deepEqual(await bundleInspection.readPhase05DeclaredVersions(repositoryRoot), {
    product: { declared: "0.0.0", authority: "VERSION + apps/desktop/package.json + tauri.conf.json" },
    runtime: { declared: "0.0.0", authority: "core/contracts/version.json" },
    ipcProtocol: { declared: "1", authority: "core/contracts/version.json" },
    voiceEngineAbi: { declared: "1", authority: "core/contracts/version.json" },
    tools: {
      node: { declared: "24.16.0", authority: ".node-version" },
      pnpm: { declared: "11.19.0", authority: "package.json#packageManager" },
      rust: { declared: "1.97.1", authority: "rust-toolchain.toml" },
      cmake: { declared: "4.4.2", authority: ".github/workflows/build.yml" },
      ninja: { declared: "1.13.2", authority: ".github/workflows/build.yml" },
    },
  });
});

test("collects only passing observed tool and Info.plist versions", async () => {
  assert.equal(typeof bundleInspection.collectPhase05VersionEvidence, "function");
  const checks = [
    ["node", "24.16.0"],
    ["pnpm", "11.19.0"],
    ["rustc", "1.97.1"],
    ["cargo", "1.97.1"],
    ["cmake", "4.4.2"],
    ["ninja", "1.13.2"],
  ].map(([id, detail]) => ({ id, status: "pass", detail }));
  const versions = await bundleInspection.collectPhase05VersionEvidence(
    repositoryRoot,
    bundleRoot,
    {
      doctorRunner: () => ({ checks, exitCode: 0 }),
      toolRunner: () => "0.0.0\n",
    },
  );
  assert.deepEqual(versions.tools.rust, {
    declared: "1.97.1",
    observed: "1.97.1",
    authority: "rust-toolchain.toml",
  });
  assert.equal(versions.product.observed, "0.0.0");

  const driftedChecks = checks.map((check) =>
    check.id === "node" ? { ...check, detail: "0.0.0" } : check,
  );
  await assert.rejects(
    bundleInspection.collectPhase05VersionEvidence(repositoryRoot, bundleRoot, {
      doctorRunner: () => ({ checks: driftedChecks, exitCode: 0 }),
      toolRunner: () => "0.0.0\n",
    }),
    /observed version drift for node/u,
  );
});
